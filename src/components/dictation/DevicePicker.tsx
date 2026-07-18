import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

/**
 * Sentinel `value` for "system default" in the Select. Radix `SelectItem`
 * forbids an empty-string value, so the empty-string device setting (which the
 * backend reads as "system default", D6) is represented by this sentinel in the
 * widget and mapped back to `""` on change.
 */
const SYSTEM_DEFAULT = "__system_default__";

interface DevicePickerProps {
  /** The saved device name; `""` = system default. */
  value: string;
  /** Enumerated input-device names from `list_audio_input_devices`. */
  devices: string[];
  /** Persist the choice (`""` for system default) via `set_dictation_setting`. */
  onChange: (device: string) => void;
  disabled?: boolean;
}

/**
 * Microphone picker (D8). «Системный по умолчанию» is always the first item and
 * maps to the empty-string setting. When the saved device is not present in the
 * current enumeration — unplugged since it was pinned — it is still listed, with
 * a «(недоступно)» suffix, so the UI mirrors the backend rollback (device not
 * found at start → system default + a `warn`) instead of silently snapping the
 * selection to default.
 */
export function DevicePicker({ value, devices, onChange, disabled }: DevicePickerProps) {
  const savedMissing = value !== "" && !devices.includes(value);

  return (
    <Select
      value={value === "" ? SYSTEM_DEFAULT : value}
      onValueChange={(v) => onChange(v === SYSTEM_DEFAULT ? "" : v)}
      disabled={disabled}
    >
      <SelectTrigger className="w-full">
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        <SelectItem value={SYSTEM_DEFAULT}>Системный по умолчанию</SelectItem>
        {devices.map((name) => (
          <SelectItem key={name} value={name}>
            {name}
          </SelectItem>
        ))}
        {savedMissing && (
          <SelectItem value={value}>{value} (недоступно)</SelectItem>
        )}
      </SelectContent>
    </Select>
  );
}
