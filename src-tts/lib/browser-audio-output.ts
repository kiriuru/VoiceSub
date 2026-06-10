import { ttsTrace } from "./tts-trace";

export type BrowserAudioOutput = {
  id: string;
  label: string;
  is_default?: boolean;
};

type MediaDevicesWithPicker = MediaDevices & {
  selectAudioOutput?: () => Promise<MediaDeviceInfo>;
};

export function isBrowserAudioOutputSupported(): boolean {
  if (typeof navigator === "undefined" || !navigator.mediaDevices) return false;
  return typeof (navigator.mediaDevices as MediaDevicesWithPicker).selectAudioOutput === "function";
}

export function isSetSinkIdSupported(): boolean {
  if (typeof Audio === "undefined") return false;
  return "setSinkId" in Audio.prototype;
}

/** List render devices for the dropdown (`enumerateDevices`, audiooutput). */
export async function listBrowserAudioOutputs(): Promise<BrowserAudioOutput[]> {
  const outputs: BrowserAudioOutput[] = [
    { id: "", label: "Default (system)", is_default: true },
  ];
  if (!navigator.mediaDevices?.enumerateDevices) {
    return outputs;
  }
  try {
    const devices = await navigator.mediaDevices.enumerateDevices();
    const seen = new Set<string>();
    let index = 1;
    for (const device of devices) {
      if (device.kind !== "audiooutput") continue;
      const id = device.deviceId || "";
      if (!id || seen.has(id)) continue;
      seen.add(id);
      const label = device.label?.trim() || `Audio output ${index}`;
      outputs.push({ id, label });
      index += 1;
    }
    ttsTrace("audio", "enumerate_outputs", { count: outputs.length - 1 });
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    ttsTrace("audio", "enumerate_failed", { message });
  }
  return outputs;
}

export async function promptBrowserAudioOutput(): Promise<MediaDeviceInfo | null> {
  if (!isBrowserAudioOutputSupported()) {
    ttsTrace("audio", "select_output_unsupported", {});
    return null;
  }
  try {
    const device = await (navigator.mediaDevices as MediaDevicesWithPicker).selectAudioOutput!();
    ttsTrace("audio", "select_output_ok", {
      device_id: device.deviceId,
      label: device.label,
    });
    return device;
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    ttsTrace("audio", "select_output_cancelled", { message });
    throw err;
  }
}

export async function resolveBrowserAudioLabel(deviceId: string): Promise<string> {
  if (!deviceId || !navigator.mediaDevices?.enumerateDevices) return "";
  try {
    const devices = await navigator.mediaDevices.enumerateDevices();
    const match = devices.find((d) => d.kind === "audiooutput" && d.deviceId === deviceId);
    return match?.label ?? "";
  } catch {
    return "";
  }
}
