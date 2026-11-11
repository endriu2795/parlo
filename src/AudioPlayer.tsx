import { t } from "./i18n";
import { useEffect, useRef, useState, type CSSProperties } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import {
  Headphones,
  Pause,
  Play,
  RotateCcw,
  RotateCw,
  Volume2,
  VolumeX,
  Video,
  EyeOff,
  Eye,
  Pin,
  PinOff,
  LocateFixed,
  ScanText,
  Maximize2,
  Minimize2,
} from "lucide-react";
import { api } from "./api";

function clock(seconds: number) {
  const value = Math.max(0, Math.floor(seconds || 0));
  return `${Math.floor(value / 60)}:${String(value % 60).padStart(2, "0")}`;
}

export default function AudioPlayer({
  callId,
  seekRequest,
  video = false,
  pinned,
  following,
  hasPassages,
  onPinChange,
  onFollowChange,
  onJumpToPassage,
  onPositionChange,
}: {
  callId: string;
  video?: boolean;
  seekRequest?: { time: number; id: number };
  pinned: boolean;
  following: boolean;
  hasPassages: boolean;
  onPinChange: (pinned: boolean) => void;
  onFollowChange: (following: boolean) => void;
  onJumpToPassage: () => void;
  onPositionChange: (time: number) => void;
}) {
  const audio = useRef<HTMLMediaElement>(null);
  const [showVideo, setShowVideo] = useState(video);
  const [largeVideo, setLargeVideo] = useState(false);
  const Media = video ? "video" : "audio";
  const [source, setSource] = useState("");
  const [error, setError] = useState("");
  const [speed, setSpeed] = useState(1);
  const [attempt, setAttempt] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [position, setPosition] = useState(0);
  const [duration, setDuration] = useState(0);
  const [volume, setVolume] = useState(1);
  const [muted, setMuted] = useState(false);
  const lastSeek = useRef<number | null>(null);
  const play = (player: HTMLMediaElement) => {
    void player.play().catch((e) => {
      if (e.name !== "AbortError" && audio.current === player)
        setError(t("Impossibile avviare la riproduzione. Riprova."));
    });
  };
  const seekPending = () => {
    const player = audio.current;
    if (
      !player ||
      !seekRequest ||
      player.readyState === 0 ||
      lastSeek.current === seekRequest.id
    )
      return;
    player.currentTime = Math.min(
      Math.max(0, seekRequest.time / 1000),
      Number.isFinite(player.duration) ? player.duration : Infinity,
    );
    lastSeek.current = seekRequest.id;
    setPosition(player.currentTime);
    onPositionChange(player.currentTime * 1000);
    play(player);
  };
  useEffect(seekPending, [seekRequest, source]);
  useEffect(() => {
    const player = audio.current;
    return () => {
      player?.pause();
      player?.removeAttribute("src");
      player?.load();
    };
  }, [source]);
  useEffect(() => {
    let alive = true;
    setSource("");
    setError("");
    setPlaying(false);
    setPosition(0);
    setDuration(0);
    lastSeek.current = null;
    void (video ? api.prepareVideo(callId) : api.prepareAudio(callId))
      .then((path) => {
        if (alive) setSource(convertFileSrc(path));
      })
      .catch((e) => {
        if (alive) setError(String(e));
      });
    return () => {
      alive = false;
      audio.current?.pause();
    };
  }, [callId, attempt, video]);
  const seek = (time: number) => {
    if (!audio.current || !duration) return;
    audio.current.currentTime = Math.min(duration, Math.max(0, time));
    setPosition(audio.current.currentTime);
    onPositionChange(audio.current.currentTime * 1000);
  };
  return (
    <section
      className={`call-audio-player ${playing ? "is-playing" : ""} ${largeVideo ? "video-expanded" : ""}`}
      aria-label={video ? t("Video con audio della registrazione") : t("Audio completo della registrazione")}
    >
      <div className="call-audio-heading">
        <strong>
          {video ? <Video size={15}/> : <Headphones size={15}/>} {video ? t("Video con audio") : "Audio"}
        </strong>
        {video && <button type="button" className="text-button media-view-toggle" aria-pressed={showVideo} onClick={() => setShowVideo(!showVideo)}>{showVideo ? <EyeOff size={14}/> : <Eye size={14}/>} {showVideo ? t("Solo ascolto") : t("Mostra video")}</button>}
        {video && pinned && showVideo && <button type="button" className="text-button" onClick={() => setLargeVideo(!largeVideo)} title={largeVideo ? t("Riduci il video fissato") : t("Ingrandisci il video fissato")}>{largeVideo ? <Minimize2 size={14}/> : <Maximize2 size={14}/>} {largeVideo ? t("Riduci") : t("Ingrandisci")}</button>}
        <select
          aria-label={t("Velocità di riproduzione")}
          title={t("Velocità di riproduzione")}
          value={speed}
          onChange={(e) => {
            const value = Number(e.target.value);
            setSpeed(value);
            if (audio.current) audio.current.playbackRate = value;
          }}
        >
          {[0.75, 1, 1.25, 1.5, 2].map((value) => (
            <option key={value} value={value}>
              {value}×
            </option>
          ))}
        </select>
      </div>
      {error ? (
        <p role="alert">
          {error}{" "}
          <button
            className="text-button"
            onClick={() => setAttempt((value) => value + 1)}
          >
            {t("Riprova")}
          </button>
        </p>
      ) : source ? (
        <>
          <Media
            ref={(element: HTMLAudioElement | HTMLVideoElement | null) => { audio.current = element; }}
            className="recording-video"
            preload="metadata"
            src={source}
            hidden={!video || !showVideo}
            onLoadedMetadata={(e) => {
              const player = e.currentTarget;
              player.playbackRate = speed;
              player.volume = volume;
              player.muted = muted;
              setDuration(
                Number.isFinite(player.duration) ? player.duration : 0,
              );
              onPositionChange(player.currentTime * 1000);
              seekPending();
            }}
            onDurationChange={(e) =>
              setDuration(
                Number.isFinite(e.currentTarget.duration)
                  ? e.currentTarget.duration
                  : 0,
              )
            }
            onTimeUpdate={(e) => { setPosition(e.currentTarget.currentTime); onPositionChange(e.currentTarget.currentTime * 1000); }}
            onPlay={() => setPlaying(true)}
            onPause={() => setPlaying(false)}
            onEnded={() => setPlaying(false)}
            onError={() =>
              setError(t("Impossibile riprodurre la registrazione conservata."))
            }
          />
          <div className="audio-transport">
            <button
              className="audio-play"
              aria-label={playing ? t("Pausa") : t("Riproduci")}
              onClick={() => {
                const player = audio.current;
                if (player) {
                  if (player.paused) play(player);
                  else player.pause();
                }
              }}
            >
              <svg className="audio-play-disc" width="34" height="34" viewBox="0 0 34 34" aria-hidden="true">
                <circle cx="17" cy="17" r="16" />
              </svg>
              {playing ? (
                <Pause size={18} fill="currentColor" />
              ) : (
                <Play size={18} fill="currentColor" />
              )}
            </button>
            <button
              className="audio-skip"
              aria-label={t("Indietro di 10 secondi")}
              title={t("Indietro di 10 secondi")}
              disabled={!duration}
              onClick={() => seek(position - 10)}
            >
              <RotateCcw size={16} />
              <span>10</span>
            </button>
            <div className="audio-timeline">
              <input
                className="audio-progress"
                type="range"
                min={0}
                max={duration || 1}
                step="0.1"
                value={Math.min(position, duration)}
                disabled={!duration}
                aria-label={t("Posizione nella registrazione")}
                aria-valuetext={t("{0} di {1}", clock(position), clock(duration))}
                style={
                  {
                    "--progress": `${duration ? (position / duration) * 100 : 0}%`,
                  } as CSSProperties
                }
                onChange={(e) => seek(Number(e.target.value))}
              />
              <div className="audio-times">
                <span>{clock(position)}</span>
                <span>{clock(duration)}</span>
              </div>
            </div>
            <button
              className="audio-skip"
              aria-label={t("Avanti di 10 secondi")}
              title={t("Avanti di 10 secondi")}
              disabled={!duration}
              onClick={() => seek(position + 10)}
            >
              <RotateCw size={16} />
              <span>10</span>
            </button>
            <div className="audio-volume">
              <button
                aria-label={muted ? t("Riattiva audio") : t("Disattiva audio")}
                title={muted ? t("Riattiva audio") : t("Disattiva audio")}
                onClick={() => {
                  setMuted(!muted);
                  if (audio.current) audio.current.muted = !muted;
                }}
              >
                {muted || volume === 0 ? (
                  <VolumeX size={17} />
                ) : (
                  <Volume2 size={17} />
                )}
              </button>
              <input
                type="range"
                min={0}
                max={1}
                step="0.05"
                value={muted ? 0 : volume}
                aria-label="Volume"
                onChange={(e) => {
                  const value = Number(e.target.value);
                  setVolume(value);
                  setMuted(false);
                  if (audio.current) {
                    audio.current.volume = value;
                    audio.current.muted = false;
                  }
                }}
              />
            </div>
          </div>
          <div className="media-navigation" aria-label={t("Navigazione trascrizione")}>
            <button type="button" className="text-button" disabled={!hasPassages || !duration} onClick={onJumpToPassage} title={t("Vai al passaggio corrispondente o più vicino alla riproduzione")}><LocateFixed size={14}/> {t("Vai al punto")}</button>
            <button type="button" className={`text-button ${following ? "selected" : ""}`} aria-pressed={following} disabled={!hasPassages || !duration} onClick={() => onFollowChange(!following)} title={t("Segui automaticamente il passaggio in riproduzione")}><ScanText size={14}/> {following ? t("Segui: attivo") : t("Segui")}</button>
            <button type="button" className={`text-button ${pinned ? "selected" : ""}`} aria-pressed={pinned} onClick={() => onPinChange(!pinned)} title={pinned ? t("Sblocca il player") : t("Fissa il player durante lo scorrimento")}>{pinned ? <PinOff size={14}/> : <Pin size={14}/>} {pinned ? t("Sblocca") : t("Fissa")}</button>
          </div>
        </>
      ) : (
        <p className="audio-loading" role="status">
          {t("Preparazione registrazione…")}
        </p>
      )}
    </section>
  );
}
