// Audio only. Both streams share the host clock; no screen output is persisted.
#import <Foundation/Foundation.h>
#import <AppKit/AppKit.h>
#import <AVFoundation/AVFoundation.h>
#import <ScreenCaptureKit/ScreenCaptureKit.h>
#import <CoreMedia/CoreMedia.h>
#import <CoreAudio/CoreAudio.h>
#import <CoreGraphics/CoreGraphics.h>
#import <UserNotifications/UserNotifications.h>
#include <stdlib.h>
#include <stdio.h>
#include <stdint.h>
#include <math.h>
#include <string.h>
#include <stdbool.h>

bool parlo_uses_italian(void) {
    NSString *preferred = [NSLocale preferredLanguages].firstObject;
    if (!preferred) return false;
    NSString *language = [NSLocale componentsFromLocaleIdentifier:preferred][NSLocaleLanguageCode];
    return [language isEqualToString:@"it"];
}

typedef void (*ParloCallback)(void *, const char *);
char *parlo_pick_sources(bool folder, bool media) {
    NSOpenPanel *panel = [NSOpenPanel openPanel];
    if (parlo_uses_italian()) {
        panel.title = folder ? @"Scegli una cartella" : media ? @"Importa audio o video" : @"Aggiungi file a questa chat";
    } else {
        panel.title = folder ? @"Choose a folder" : media ? @"Import audio or video" : @"Add files to this chat";
    }
    panel.canChooseFiles = !folder;
    panel.canChooseDirectories = folder;
    panel.allowsMultipleSelection = !folder && !media;
    if ([panel runModal] != NSModalResponseOK) return NULL;
    NSMutableArray *paths = [NSMutableArray array];
    for (NSURL *url in panel.URLs) [paths addObject:url.path];
    NSData *data = [NSJSONSerialization dataWithJSONObject:paths options:0 error:nil];
    if (!data) return NULL;
    NSString *json = [[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding];
    return strdup(json.UTF8String);
}
char *parlo_save_markdown(const char *name) {
    NSSavePanel *panel = [NSSavePanel savePanel];
    panel.title = parlo_uses_italian() ? @"Salva file" : @"Save file";
    panel.nameFieldStringValue = [NSString stringWithUTF8String:name];
    panel.canCreateDirectories = YES;
    if ([panel runModal] != NSModalResponseOK || !panel.URL) return NULL;
    return strdup(panel.URL.path.fileSystemRepresentation);
}
static double hostSeconds(void) { return CMTimeGetSeconds(CMClockGetTime(CMClockGetHostTimeClock())); }
static void little32(uint8_t *p, uint32_t v) { for (int i=0;i<4;i++) p[i]=(v>>(8*i))&255; }
static void little16(uint8_t *p, uint16_t v) { p[0]=v&255; p[1]=v>>8; }

@interface ParloTrack : NSObject
@property FILE *file;
@property uint64_t frames;
@property double origin;
@property double lastHeader;
@property double lastLevel;
@property uint64_t missing;
@property NSUInteger reconnections;
@property double unavailable;
@property NSMutableArray<NSString *> *positions;
@property AVAudioConverter *converter;
@property AVAudioFormat *inputFormat;
@property NSString *source;
@property(copy) void (^emit)(NSDictionary *);
- (instancetype)initWithPath:(NSString *)path source:(NSString *)source origin:(double)origin emit:(void (^)(NSDictionary *))emit;
- (void)append:(AVAudioPCMBuffer *)buffer time:(double)timestamp;
- (NSString *)finish:(double)end;
@end
@implementation ParloTrack
- (instancetype)initWithPath:(NSString *)path source:(NSString *)source origin:(double)origin emit:(void (^)(NSDictionary *))emit {
    if ((self=[super init])) {
        _file=fopen(path.fileSystemRepresentation,"wb+");
        if (!_file) return nil;
        _source=source; _origin=origin; _emit=[emit copy]; _positions=[NSMutableArray new];
        [self header];
    } return self;
}
- (void)header {
    uint8_t h[44]={0}; memcpy(h,"RIFF",4); little32(h+4,(uint32_t)(_frames*2+36));
    memcpy(h+8,"WAVEfmt ",8); little32(h+16,16); little16(h+20,1); little16(h+22,1);
    little32(h+24,16000); little32(h+28,32000); little16(h+32,2); little16(h+34,16);
    memcpy(h+36,"data",4); little32(h+40,(uint32_t)(_frames*2));
    fseek(_file,0,SEEK_SET); fwrite(h,1,44,_file); fseek(_file,0,SEEK_END); fflush(_file);
    _lastHeader=hostSeconds();
}
- (void)zeros:(uint64_t)count {
    uint8_t zeros[8192]={0};
    while(count) { size_t n=(size_t)MIN(count,4096); if(fwrite(zeros,2,n,_file)!=n) break; _frames+=n; count-=n; }
}
- (void)append:(AVAudioPCMBuffer *)buffer time:(double)timestamp {
    if (!_file || !buffer.frameLength) return;
    if (!_converter || ![_inputFormat isEqual:buffer.format]) {
        _inputFormat=buffer.format;
        AVAudioFormat *format=[[AVAudioFormat alloc] initWithCommonFormat:AVAudioPCMFormatFloat32 sampleRate:16000 channels:1 interleaved:NO];
        _converter=[[AVAudioConverter alloc] initFromFormat:_inputFormat toFormat:format];
        _converter.primeMethod=AVAudioConverterPrimeMethod_None;
    }
    if (!_converter) { _emit(@{@"type":@"error",@"message":@"Formato audio Mac non supportato."}); return; }
    AVAudioFrameCount capacity=(AVAudioFrameCount)ceil(buffer.frameLength*16000.0/buffer.format.sampleRate)+32;
    AVAudioPCMBuffer *output=[[AVAudioPCMBuffer alloc] initWithPCMFormat:_converter.outputFormat frameCapacity:capacity];
    __block BOOL supplied=NO; NSError *error=nil;
    [_converter convertToBuffer:output error:&error withInputFromBlock:^AVAudioBuffer *(AVAudioPacketCount count, AVAudioConverterInputStatus *status) {
        if (supplied) { *status=AVAudioConverterInputStatus_NoDataNow; return nil; }
        supplied=YES; *status=AVAudioConverterInputStatus_HaveData; return buffer;
    }];
    if (error) { _emit(@{@"type":@"error",@"message":error.localizedDescription}); return; }
    if (!output.frameLength) return;
    uint64_t expected=_frames;
    // Invalid clocks must never truncate speech or create enormous silent files.
    if (isfinite(timestamp) && timestamp>=_origin && timestamp<=hostSeconds()+1)
        expected=(uint64_t)llround((timestamp-_origin)*16000);
    uint64_t gap=expected>_frames+32 ? expected-_frames : 0;
    if (_frames && gap>320 && [_source isEqualToString:@"microphone"]) {
        _missing+=gap;
        if (_positions.count<6) [_positions addObject:[NSString stringWithFormat:@"%02llu:%02llu",_frames/16000/60,_frames/16000%60]];
    }
    [self zeros:gap];
    size_t skip=(size_t)MIN(_frames>expected ? _frames-expected : 0, output.frameLength);
    float *samples=output.floatChannelData[0];
    NSMutableData *pcm=[NSMutableData dataWithLength:(output.frameLength-skip)*2];
    uint8_t *bytes=pcm.mutableBytes; double sum=0; size_t count=0;
    for(size_t i=skip;i<output.frameLength;i++) {
        float value=isfinite(samples[i]) ? fmaxf(-1,fminf(1,samples[i])) : 0;
        little16(bytes+(i-skip)*2,(uint16_t)(int16_t)lrintf(value*32767)); sum+=value*value; count++;
    }
    if (count && fwrite(bytes,2,count,_file)!=count) { _emit(@{@"type":@"error",@"message":@"Impossibile scrivere l’audio temporaneo. Controlla lo spazio su disco."}); return; }
    _frames+=count;
    double now=hostSeconds();
    if(now-_lastHeader>=1) [self header];
    if(now-_lastLevel>=0.15) {
        double db=sum>0 && count ? 20*log10(sqrt(sum/count)) : -60; db=fmax(-60,fmin(0,db));
        _emit(@{@"type":@"level",@"source":_source,@"dbfs":@(db),@"level":@((db+60)/60)}); _lastLevel=now;
    }
}
- (NSString *)finish:(double)end {
    if (_file) { uint64_t target=(uint64_t)fmax(0,(end-_origin)*16000); if(target>_frames) [self zeros:target-_frames]; [self header]; fclose(_file); _file=NULL; }
    NSMutableArray<NSString *> *warnings=[NSMutableArray new];
    if (_missing) [warnings addObject:[NSString stringWithFormat:@"Microfono: %.2f s senza campioni; verifica in prossimità di %@.",_missing/16000.0,[_positions componentsJoinedByString:@", "]]];
    if (_reconnections || _unavailable>0) [warnings addObject:[NSString stringWithFormat:@"%@: %lu riconnessioni, %.2f s in attesa del dispositivo.",[_source isEqualToString:@"microphone"] ? @"Microfono" : @"Audio PC",(unsigned long)_reconnections,_unavailable]];
    return [warnings componentsJoinedByString:@" "];
}
- (void)dealloc { if(_file) fclose(_file); }
@end

@interface ParloSession : NSObject <SCStreamOutput,SCStreamDelegate>
@property(nonatomic, strong) dispatch_queue_t queue;
@property(nonatomic, strong) AVAudioEngine *engine;
@property(nonatomic, strong) SCStream *stream;
@property(nonatomic, strong) ParloTrack *mic;
@property(nonatomic, strong) ParloTrack *system;
@property(nonatomic, strong) NSString *directory;
@property BOOL wantMic;
@property BOOL wantSystem;
@property(atomic) BOOL stopped;
@property BOOL tapInstalled;
@property BOOL tracksStarted;
@property BOOL ready;
@property BOOL microphoneRetry;
@property BOOL systemRetry;
@property BOOL systemStarting;
@property BOOL microphoneReconnecting;
@property BOOL systemReconnecting;
@property NSUInteger microphoneGeneration;
@property NSUInteger systemGeneration;
@property AudioDeviceID microphoneDevice;
@property double microphoneLostAt;
@property double systemLostAt;
@property double origin;
@property ParloCallback callback;
@property void *context;
@property(nonatomic, strong) id activity;
@property(nonatomic, strong) id engineObserver;
@property(nonatomic, strong) dispatch_source_t recoveryTimer;
- (void)prepare;
- (BOOL)beginTracks;
- (void)startSystem:(BOOL)recovering;
- (NSString *)openMicrophone;
- (void)closeMicrophone;
- (void)installRecovery;
- (void)setRoute:(NSString *)source reconnecting:(BOOL)reconnecting;
- (NSString *)stop;
@end
static AudioDeviceID defaultInputDevice(void) {
    AudioDeviceID device=kAudioObjectUnknown;
    UInt32 size=sizeof(device);
    AudioObjectPropertyAddress address={kAudioHardwarePropertyDefaultInputDevice,kAudioObjectPropertyScopeGlobal,kAudioObjectPropertyElementMain};
    if(AudioObjectGetPropertyData(kAudioObjectSystemObject,&address,0,NULL,&size,&device)!=noErr) return kAudioObjectUnknown;
    return device;
}
@implementation ParloSession
- (void)emit:(NSDictionary *)value {
    // All C callbacks run here. stop() drains this queue before Rust frees context.
    if(self.stopped) return;
    NSData *data=[NSJSONSerialization dataWithJSONObject:value options:0 error:nil];
    NSString *json=[[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding];
    if(json) _callback(_context,json.UTF8String);
}
- (void)error:(NSString *)message { [self emit:@{@"type":@"error",@"message":message ?: @"Cattura audio Mac interrotta."}]; }
- (void)setRoute:(NSString *)source reconnecting:(BOOL)reconnecting {
    BOOL mic=[source isEqualToString:@"microphone"];
    BOOL previous=mic ? _microphoneReconnecting : _systemReconnecting;
    if(previous!=reconnecting) {
        double now=hostSeconds();
        double since=mic ? _microphoneLostAt : _systemLostAt;
        ParloTrack *track=mic ? _mic : _system;
        if(reconnecting) { track.reconnections++; since=now; }
        else if(since>0) { track.unavailable+=now-since; since=0; }
        if(mic) { _microphoneReconnecting=reconnecting; _microphoneLostAt=since; }
        else { _systemReconnecting=reconnecting; _systemLostAt=since; }
    }
    [self emit:@{@"type":@"route",@"source":source,@"reconnecting":@(reconnecting)}];
}
- (void)prepare {
    if (self.stopped) return;
    if (_wantMic && [AVCaptureDevice authorizationStatusForMediaType:AVMediaTypeAudio]==AVAuthorizationStatusNotDetermined) {
        __weak ParloSession *weak=self;
        [AVCaptureDevice requestAccessForMediaType:AVMediaTypeAudio completionHandler:^(BOOL granted) {
            ParloSession *session=weak; if(session) dispatch_async(session.queue,^{ [session prepare]; });
        }]; return;
    }
    if (_wantMic && [AVCaptureDevice authorizationStatusForMediaType:AVMediaTypeAudio]!=AVAuthorizationStatusAuthorized) {
        [self error:@"macOS non concede l’accesso al microfono. Se in Privacy e sicurezza il permesso è già attivo, usa Impostazioni → Registrazione → Ripristina permessi e riavvia PARLO per rimuovere l’autorizzazione legata alla versione precedente."]; return;
    }
    if(_wantSystem) [self startSystem:NO];
    else if([self beginTracks]) { _ready=YES; [self emit:@{@"type":@"ready"}]; }
}
- (BOOL)recoverableSystemError:(NSError *)error {
    if(![error.domain isEqualToString:SCStreamErrorDomain]) return NO;
    // Retry transient native failures, never a user stop or permission refusal.
    return error.code==SCStreamErrorSystemStoppedStream || error.code==SCStreamErrorInternalError
        || error.code==SCStreamErrorFailedToStart || error.code==SCStreamErrorFailedToStartAudioCapture;
}
- (void)startSystem:(BOOL)recovering {
    if(_stopped || _systemStarting) return;
    _systemStarting=YES; _systemRetry=NO;
    NSUInteger generation=++_systemGeneration;
    if(recovering) {
        __weak ParloSession *weak=self;
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW,10*NSEC_PER_SEC),_queue,^{
            ParloSession *session=weak;
            if(!session || session.stopped || generation!=session.systemGeneration || !session.systemStarting) return;
            SCStream *abandoned=session.stream; session.stream=nil;
            ++session.systemGeneration; session.systemStarting=NO; session.systemRetry=YES;
            if(abandoned) [abandoned stopCaptureWithCompletionHandler:^(NSError *error) {}];
        });
    }
    [SCShareableContent getShareableContentExcludingDesktopWindows:YES onScreenWindowsOnly:YES completionHandler:^(SCShareableContent *content, NSError *error) {
        dispatch_async(self.queue,^{
            if(self.stopped || generation!=self.systemGeneration) return;
            if(error || !content.displays.count) {
                self.systemStarting=NO;
                if(recovering && CGPreflightScreenCaptureAccess()) { self.systemRetry=YES; return; }
                if(error.code==SCStreamErrorUserDeclined && [error.domain isEqualToString:SCStreamErrorDomain]) {
                    [self error:@"macOS non concede la registrazione schermo a PARLO. Se in Privacy e sicurezza il permesso è già attivo, usa Impostazioni → Registrazione → Ripristina permessi e riavvia PARLO per rimuovere l’autorizzazione legata alla versione precedente."];
                } else {
                    [self error:[NSString stringWithFormat:@"Avvio dell’audio del computer non riuscito: %@", error.localizedDescription ?: @"nessun display disponibile"]];
                }
                return;
            }
            if(!self.tracksStarted && ![self beginTracks]) { self.systemStarting=NO; return; }
            SCContentFilter *filter=[[SCContentFilter alloc] initWithDisplay:content.displays.firstObject excludingWindows:@[]];
            SCStreamConfiguration *config=[SCStreamConfiguration new]; config.capturesAudio=YES; config.excludesCurrentProcessAudio=YES;
            config.sampleRate=16000; config.channelCount=1;
            config.width=2; config.height=2; config.minimumFrameInterval=CMTimeMake(1,1); config.showsCursor=NO;
            SCStream *stream=[[SCStream alloc] initWithFilter:filter configuration:config delegate:self];
            if(recovering) { self.system.converter=nil; self.system.inputFormat=nil; }
            NSError *failure=nil;
            BOOL added=[stream addStreamOutput:self type:SCStreamOutputTypeAudio sampleHandlerQueue:self.queue error:&failure]
                && [stream addStreamOutput:self type:SCStreamOutputTypeScreen sampleHandlerQueue:self.queue error:&failure];
            if(!added) { self.systemStarting=NO; [self error:failure.localizedDescription]; return; }
            self.stream=stream;
            [stream startCaptureWithCompletionHandler:^(NSError *failure) {
                dispatch_async(self.queue,^{
                    if(self.stopped || generation!=self.systemGeneration) return;
                    self.systemStarting=NO;
                    if(failure) {
                        self.stream=nil;
                        if(recovering && [self recoverableSystemError:failure]) { self.systemRetry=YES; return; }
                        [self error:failure.localizedDescription]; return;
                    }
                    [self setRoute:@"system" reconnecting:NO];
                    if(!self.ready) { self.ready=YES; [self emit:@{@"type":@"ready"}]; }
                });
            }];
        });
    }];
}
- (BOOL)beginTracks {
    if(_tracksStarted) return YES;
    self.origin=hostSeconds();
    self.activity=[[NSProcessInfo processInfo] beginActivityWithOptions:NSActivityUserInitiated|NSActivityIdleSystemSleepDisabled reason:@"Registrazione call PARLO"];
    __weak ParloSession *weak=self;
    void (^emit)(NSDictionary *)=^(NSDictionary *value) { [weak emit:value]; };
    if(_wantSystem) _system=[[ParloTrack alloc] initWithPath:[_directory stringByAppendingPathComponent:@"system.wav"] source:@"system" origin:_origin emit:emit];
    if(_wantMic) _mic=[[ParloTrack alloc] initWithPath:[_directory stringByAppendingPathComponent:@"microphone.wav"] source:@"microphone" origin:_origin emit:emit];
    if((_wantMic && !_mic) || (_wantSystem && !_system)) { [self error:@"Impossibile creare i file audio temporanei."]; return NO; }
    if(_wantMic) { NSString *error=[self openMicrophone]; if(error) { [self error:error]; return NO; } }
    _tracksStarted=YES;
    [self installRecovery];
    return YES;
}
- (void)closeMicrophone {
    ++_microphoneGeneration;
    AVAudioEngine *engine=_engine; _engine=nil;
    BOOL tapped=_tapInstalled; _tapInstalled=NO;
    // A disappearing device can invalidate its input node during teardown.
    @try { if(tapped) [engine.inputNode removeTapOnBus:0]; }
    @catch(NSException *exception) { /* The old engine is discarded below. */ }
    @try { [engine stop]; }
    @catch(NSException *exception) { /* Reconnection uses a fresh engine. */ }
}
- (NSString *)openMicrophone {
    [self closeMicrophone];
    _mic.converter=nil; _mic.inputFormat=nil;
    @try {
    _microphoneDevice=defaultInputDevice();
    if(_microphoneDevice==kAudioObjectUnknown) return @"Nessun microfono disponibile sul Mac.";
    _engine=[AVAudioEngine new]; AVAudioInputNode *input=_engine.inputNode; AVAudioFormat *format=[input outputFormatForBus:0];
    if(!format.channelCount || !format.sampleRate) { [self closeMicrophone]; return @"Microfono temporaneamente non disponibile."; }
    NSUInteger generation=_microphoneGeneration;
    __weak ParloSession *weak=self;
    [input installTapOnBus:0 bufferSize:2048 format:format block:^(AVAudioPCMBuffer *buffer, AVAudioTime *when) {
        ParloSession *session=weak; if(!session || session.stopped) return;
        AVAudioPCMBuffer *copy=[[AVAudioPCMBuffer alloc] initWithPCMFormat:buffer.format frameCapacity:buffer.frameLength];
        if(!copy) return;
        copy.frameLength=buffer.frameLength;
        const AudioBufferList *src=buffer.audioBufferList; AudioBufferList *dst=copy.mutableAudioBufferList;
        for(UInt32 i=0;i<src->mNumberBuffers;i++) memcpy(dst->mBuffers[i].mData,src->mBuffers[i].mData,src->mBuffers[i].mDataByteSize);
        double time=when.hostTimeValid ? [AVAudioTime secondsForHostTime:when.hostTime] : NAN;
        dispatch_async(session.queue,^{ if(!session.stopped && generation==session.microphoneGeneration) [session.mic append:copy time:time]; });
    }];
    _tapInstalled=YES; NSError *error=nil;
    if(![_engine startAndReturnError:&error]) { [self closeMicrophone]; return error.localizedDescription ?: @"Microfono non disponibile."; }
    return nil;
    } @catch(NSException *exception) {
        [self closeMicrophone];
        return exception.reason ?: @"Microfono temporaneamente non disponibile.";
    }
}
- (void)installRecovery {
    __weak ParloSession *weak=self;
    _engineObserver=[[NSNotificationCenter defaultCenter] addObserverForName:AVAudioEngineConfigurationChangeNotification object:nil queue:nil usingBlock:^(NSNotification *notification) {
        ParloSession *session=weak; if(!session) return;
        // Apple delivers this on an internal engine queue. Never tear the
        // engine down there: schedule recovery on our serial control queue.
        AVAudioEngine *changed=notification.object;
        dispatch_async(session.queue,^{
            if(!session.stopped && session.engine==changed && !changed.isRunning) {
                session.microphoneRetry=YES;
                [session setRoute:@"microphone" reconnecting:YES];
            }
        });
    }];
    _recoveryTimer=dispatch_source_create(DISPATCH_SOURCE_TYPE_TIMER,0,0,_queue);
    dispatch_source_set_timer(_recoveryTimer,dispatch_time(DISPATCH_TIME_NOW,NSEC_PER_SEC/2),NSEC_PER_SEC/2,NSEC_PER_SEC/10);
    dispatch_source_set_event_handler(_recoveryTimer,^{
        ParloSession *session=weak; if(!session || session.stopped) return;
        if(session.wantMic) {
            if([AVCaptureDevice authorizationStatusForMediaType:AVMediaTypeAudio]!=AVAuthorizationStatusAuthorized) {
                [session error:@"Il permesso microfono è stato revocato. Autorizzalo nelle Impostazioni di Sistema prima di registrare di nuovo."];
                return;
            }
            BOOL changed=defaultInputDevice()!=session.microphoneDevice;
            if(changed || !session.engine.isRunning || session.microphoneRetry) {
                [session setRoute:@"microphone" reconnecting:YES];
                NSString *error=[session openMicrophone];
                session.microphoneRetry=(error!=nil);
                if(!error) [session setRoute:@"microphone" reconnecting:NO];
            } else [session setRoute:@"microphone" reconnecting:NO];
        }
        if(session.wantSystem) {
            if(session.systemRetry && !session.systemStarting) [session startSystem:YES];
            [session setRoute:@"system" reconnecting:session.systemReconnecting];
        }
    });
    dispatch_resume(_recoveryTimer);
}
- (void)stream:(SCStream *)stream didOutputSampleBuffer:(CMSampleBufferRef)sample ofType:(SCStreamOutputType)type {
    if(self.stopped || stream!=self.stream || type!=SCStreamOutputTypeAudio || !CMSampleBufferIsValid(sample)) return;
    CMItemCount frames=CMSampleBufferGetNumSamples(sample); if(frames<=0) return;
    CMAudioFormatDescriptionRef description=CMSampleBufferGetFormatDescription(sample);
    const AudioStreamBasicDescription *asbd=CMAudioFormatDescriptionGetStreamBasicDescription(description); if(!asbd) return;
    AVAudioFormat *format=[[AVAudioFormat alloc] initWithStreamDescription:asbd];
    AVAudioPCMBuffer *buffer=[[AVAudioPCMBuffer alloc] initWithPCMFormat:format frameCapacity:(AVAudioFrameCount)frames]; buffer.frameLength=(AVAudioFrameCount)frames;
    OSStatus status=CMSampleBufferCopyPCMDataIntoAudioBufferList(sample,0,(int32_t)frames,buffer.mutableAudioBufferList);
    if(status!=noErr) { [self error:@"Pacchetto audio del computer non valido."]; return; }
    [_system append:buffer time:CMTimeGetSeconds(CMSampleBufferGetPresentationTimeStamp(sample))];
}
- (void)stream:(SCStream *)stream didStopWithError:(NSError *)error {
    dispatch_async(_queue,^{
        if(self.stopped || stream!=self.stream) return;
        self.stream=nil; self.systemStarting=NO; ++self.systemGeneration;
        if(![self recoverableSystemError:error]) { [self error:[NSString stringWithFormat:@"Audio del computer interrotto: %@",error.localizedDescription]]; return; }
        self.systemRetry=YES;
        [self setRoute:@"system" reconnecting:YES];
    });
}
- (NSString *)stop {
    __block NSString *warning=@"";
    dispatch_sync(_queue,^{
        self.stopped=YES;
        if(self.engineObserver) { [[NSNotificationCenter defaultCenter] removeObserver:self.engineObserver]; self.engineObserver=nil; }
        if(self.recoveryTimer) { dispatch_source_cancel(self.recoveryTimer); self.recoveryTimer=nil; }
        [self closeMicrophone]; ++self.systemGeneration;
        if(self.activity) { [[NSProcessInfo processInfo] endActivity:self.activity]; self.activity=nil; }
        SCStream *stream=self.stream; self.stream=nil;
        if(stream) [stream stopCaptureWithCompletionHandler:^(NSError *error) {}];
        double end=hostSeconds();
        if(self.microphoneLostAt>0) self.mic.unavailable+=end-self.microphoneLostAt;
        if(self.systemLostAt>0) self.system.unavailable+=end-self.systemLostAt;
        NSMutableArray<NSString *> *warnings=[NSMutableArray new];
        NSString *mic=[self.mic finish:end], *system=[self.system finish:end];
        if(mic.length) [warnings addObject:mic]; if(system.length) [warnings addObject:system];
        warning=[warnings componentsJoinedByString:@" "];
        self.mic=nil; self.system=nil;
    });
    return warning;
}
@end

void *parlo_capture_create(const char *dir,int mic,int system,ParloCallback callback,void *context) {
    ParloSession *session=[ParloSession new]; session.directory=[NSString stringWithUTF8String:dir];
    session.wantMic=mic; session.wantSystem=system; session.callback=callback; session.context=context;
    session.queue=dispatch_queue_create("local.parlo.audio",dispatch_queue_attr_make_with_qos_class(DISPATCH_QUEUE_SERIAL,QOS_CLASS_USER_INTERACTIVE,0));
    dispatch_async(session.queue,^{ [session prepare]; });
    return (__bridge_retained void *)session;
}
char *parlo_capture_stop(void *pointer) {
    ParloSession *session=(__bridge_transfer ParloSession *)pointer;
    return strdup([session stop].UTF8String);
}
void parlo_string_free(char *value) { free(value); }
@interface ParloNotificationDelegate : NSObject <UNUserNotificationCenterDelegate>
@end
@implementation ParloNotificationDelegate
- (void)userNotificationCenter:(UNUserNotificationCenter *)center willPresentNotification:(UNNotification *)notification withCompletionHandler:(void (^)(UNNotificationPresentationOptions options))completionHandler {
    completionHandler(UNNotificationPresentationOptionBanner|UNNotificationPresentationOptionSound);
}
@end
static ParloNotificationDelegate *notificationDelegate;
char *parlo_notify(const char *heading, const char *title) {
    NSString *headingText=[NSString stringWithUTF8String:heading];
    NSString *text=[NSString stringWithUTF8String:title];
    UNUserNotificationCenter *center=[UNUserNotificationCenter currentNotificationCenter];
    static dispatch_once_t once;
    dispatch_once(&once, ^{ notificationDelegate=[ParloNotificationDelegate new]; center.delegate=notificationDelegate; });
    dispatch_semaphore_t completed=dispatch_semaphore_create(0);
    __block NSString *failure=nil;
    [center requestAuthorizationWithOptions:UNAuthorizationOptionAlert|UNAuthorizationOptionSound completionHandler:^(BOOL granted,NSError *error) {
        if(error) { failure=error.localizedDescription; dispatch_semaphore_signal(completed); return; }
        if(!granted) { failure=@"Le notifiche di PARLO sono disattivate. Abilita gli avvisi per PARLO in Impostazioni di Sistema → Notifiche."; dispatch_semaphore_signal(completed); return; }
        UNMutableNotificationContent *content=[UNMutableNotificationContent new]; content.title=headingText; content.body=text;
        UNNotificationRequest *request=[UNNotificationRequest requestWithIdentifier:NSUUID.UUID.UUIDString content:content trigger:nil];
        [center addNotificationRequest:request withCompletionHandler:^(NSError *error) { if(error) failure=error.localizedDescription; dispatch_semaphore_signal(completed); }];
    }];
    if(dispatch_semaphore_wait(completed, dispatch_time(DISPATCH_TIME_NOW, 20*NSEC_PER_SEC))!=0) return strdup("macOS non ha confermato l’invio della notifica entro 20 secondi.");
    return failure ? strdup(failure.UTF8String) : NULL;
}
