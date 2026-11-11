# Read endpoint metadata and configured volume. Never capture audio or alter Windows settings.
$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class ParloInputCheck {
    [ComImport, Guid("BCDE0395-E52F-467C-8E3D-C4579291692E")] class Enumerator {}
    [ComImport, Guid("A95664D2-9614-4F35-A746-DE8DB63617E6"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    interface IDevices {
        void EnumAudioEndpoints(int flow, uint mask, out IntPtr devices);
        void GetDefaultAudioEndpoint(int flow, int role, out IDevice device);
        void GetDevice([MarshalAs(UnmanagedType.LPWStr)] string id, out IDevice device);
        void RegisterEndpointNotificationCallback(IntPtr callback);
        void UnregisterEndpointNotificationCallback(IntPtr callback);
    }
    [ComImport, Guid("D666063F-1587-4E43-81F1-B948E807363F"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    interface IDevice {
        void Activate(ref Guid iid, uint context, IntPtr parameters, [MarshalAs(UnmanagedType.IUnknown)] out object instance);
        void OpenPropertyStore(uint mode, out IStore store);
        void GetId([MarshalAs(UnmanagedType.LPWStr)] out string id);
        void GetState(out uint state);
    }
    [StructLayout(LayoutKind.Sequential)] struct Key { public Guid format; public uint id; }
    [StructLayout(LayoutKind.Explicit, Size=24)] struct Value {
        [FieldOffset(0)] public ushort type;
        [FieldOffset(8)] public IntPtr pointer;
    }
    [ComImport, Guid("886D8EEB-8CF2-4446-8D02-CDBA1DBDCF99"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    interface IStore {
        void GetCount(out uint count);
        void GetAt(uint index, out Key key);
        void GetValue(ref Key key, out Value value);
        void SetValue(ref Key key, ref Value value);
        void Commit();
    }
    [ComImport, Guid("5CDF2C82-841E-4546-9722-0CF74078229A"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    interface IVolume {
        void RegisterControlChangeNotify(IntPtr callback);
        void UnregisterControlChangeNotify(IntPtr callback);
        void GetChannelCount(out uint channels);
        void SetMasterVolumeLevel(float level, IntPtr context);
        void SetMasterVolumeLevelScalar(float level, IntPtr context);
        void GetMasterVolumeLevel(out float level);
        void GetMasterVolumeLevelScalar(out float level);
        void SetChannelVolumeLevel(uint channel, float level, IntPtr context);
        void SetChannelVolumeLevelScalar(uint channel, float level, IntPtr context);
        void GetChannelVolumeLevel(uint channel, out float level);
        void GetChannelVolumeLevelScalar(uint channel, out float level);
        void SetMute([MarshalAs(UnmanagedType.Bool)] bool mute, IntPtr context);
        void GetMute([MarshalAs(UnmanagedType.Bool)] out bool mute);
    }
    [DllImport("ole32.dll")] static extern int PropVariantClear(ref Value value);
    public static void Run() {
        IDevices devices=(IDevices)new Enumerator();
        foreach (int role in new []{0,2}) {
            IDevice device; devices.GetDefaultAudioEndpoint(1,role,out device);
            IStore store; device.OpenPropertyStore(0,out store);
            Key key=new Key{format=new Guid("A45C254E-DF1C-4EFD-8020-67D146A850E0"),id=14};
            Value value; store.GetValue(ref key,out value);
            string name=value.type==31 ? Marshal.PtrToStringUni(value.pointer) : "Microfono";
            PropVariantClear(ref value);
            Guid iid=typeof(IVolume).GUID; object instance; device.Activate(ref iid,23,IntPtr.Zero,out instance);
            IVolume volume=(IVolume)instance; float level; bool mute;
            volume.GetMasterVolumeLevelScalar(out level); volume.GetMute(out mute);
            Console.WriteLine("{0}: {1}; volume={2:F0}%; mute={3}",role==0?"App (console)":"Comunicazioni",name,level*100,mute);
            Marshal.ReleaseComObject(instance); Marshal.ReleaseComObject(store); Marshal.ReleaseComObject(device);
        }
        Marshal.ReleaseComObject(devices);
    }
}
'@
[ParloInputCheck]::Run()
