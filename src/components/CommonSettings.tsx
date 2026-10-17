import { Dropdown } from "@heroui/react";
import { backend } from "../api";
import { useT } from "../i18n/useT";
import { useApp } from "../state/store";
import { SwitchRow } from "./SettingsParts";

export function CommonSettings({ locked }: { locked: boolean }) {
  const s = useApp((st) => st.settings);
  const update = useApp((st) => st.updateSettings);
  const t = useT();

  const onAction = async (key: string | number) => {
    if (key === "next") {
      update({ output: { kind: "next" } });
      return;
    }
    const path = await backend.pickFolder(t("dialog.pickFolder"));
    if (path) update({ output: { kind: "folder", path } });
  };

  return (
    <div className="contents">
      <SwitchRow
        label={t("settings.keepMetadata")}
        hint={t("settings.keepMetadataHint")}
        selected={s.keepMetadata}
        disabled={locked}
        onChange={(keepMetadata) => update({ keepMetadata })}
      />
      <Dropdown>
        <Dropdown.Trigger
          isDisabled={locked}
          className="flex w-full flex-col items-start gap-0.5 rounded-xl bg-[var(--d100)] px-3 py-2 text-left outline-none data-[disabled]:opacity-50"
        >
          <span className="text-[11px] text-[var(--muted)]">{t("settings.outputFolder")}</span>
          <span className="w-full truncate text-[13px]">
            {s.output.kind === "next" ? t("settings.nextToOriginal") : s.output.path}
          </span>
        </Dropdown.Trigger>
        <Dropdown.Popover>
          <Dropdown.Menu onAction={(k) => void onAction(k)}>
            <Dropdown.Item id="next" textValue={t("settings.nextToOriginal")}>
              {t("settings.nextToOriginal")}
            </Dropdown.Item>
            <Dropdown.Item id="pick" textValue={t("settings.pickFolder")}>
              {t("settings.pickFolder")}
            </Dropdown.Item>
          </Dropdown.Menu>
        </Dropdown.Popover>
      </Dropdown>
      <SwitchRow
        label={t("settings.trashOriginals")}
        hint={t("settings.trashHint")}
        selected={s.trashOriginals}
        disabled={locked}
        onChange={(trashOriginals) => update({ trashOriginals })}
      />
    </div>
  );
}
