import { Dropdown } from "@heroui/react";
import { backend } from "../api";
import { useApp } from "../state/store";
import { SwitchRow } from "./SettingsParts";

export function CommonSettings({ locked }: { locked: boolean }) {
  const s = useApp((st) => st.settings);
  const update = useApp((st) => st.updateSettings);

  const onAction = async (key: string | number) => {
    if (key === "next") {
      update({ output: { kind: "next" } });
      return;
    }
    const path = await backend.pickFolder();
    if (path) update({ output: { kind: "folder", path } });
  };

  return (
    <div className="contents">
      <SwitchRow
        label="Metadaten behalten"
        hint="EXIF, XMP, Farbprofil"
        selected={s.keepMetadata}
        disabled={locked}
        onChange={(keepMetadata) => update({ keepMetadata })}
      />
      <Dropdown>
        <Dropdown.Trigger
          isDisabled={locked}
          className="flex w-full flex-col items-start gap-0.5 rounded-xl bg-[var(--d100)] px-3 py-2 text-left outline-none data-[disabled]:opacity-50"
        >
          <span className="text-[11px] text-[var(--muted)]">Ausgabeordner · klicken zum Ändern</span>
          <span className="w-full truncate text-[13px]">
            {s.output.kind === "next" ? "Neben der Originaldatei" : s.output.path}
          </span>
        </Dropdown.Trigger>
        <Dropdown.Popover>
          <Dropdown.Menu onAction={(k) => void onAction(k)}>
            <Dropdown.Item id="next" textValue="Neben der Originaldatei">
              Neben der Originaldatei
            </Dropdown.Item>
            <Dropdown.Item id="pick" textValue="Ordner wählen …">
              Ordner wählen …
            </Dropdown.Item>
          </Dropdown.Menu>
        </Dropdown.Popover>
      </Dropdown>
      <SwitchRow
        label="Originale nach Prüfung in den Papierkorb"
        hint="Nur bei verlustfreiem Ergebnis"
        selected={s.trashOriginals}
        disabled={locked}
        onChange={(trashOriginals) => update({ trashOriginals })}
      />
    </div>
  );
}
