# Acceptance: JPEG XL Converter

Step by step, with checkboxes. **Work on copies first.** The app never deletes anything
permanently; with the Recycle Bin switch it moves originals to the Windows Recycle Bin after a
successful check. Still: check everything on a copy first, then on the real archive.

The app is portable in `dist-portable\JPEG XL Converter\` (`JPEG XL Converter.exe`, `cjxl.exe`,
`djxl.exe` and `LICENSES\` must stay in the same folder). The folder is built with
`.\scripts\package-portable.ps1`. The `data\` folder is not part of it; it is created next to the
exe on the first start (settings, window position, logs).

The labels below are those of the **English** interface. To get them, set the language to
"English" with the sliders button in the title bar (App settings → Language). The German version
of this list is [`ABNAHME.md`](ABNAHME.md).

## 0. Preparation

- [ ] Start `JPEG XL Converter.exe` from the folder. There is **no** notice bar saying
      "cjxl.exe or djxl.exe is missing" and the start button is not disabled.
- [ ] A `data\` folder has appeared next to the exe.
- [ ] The "Move originals to Recycle Bin after check" switch is **off** at start
      (also after a restart, even if it was on before).

## 1. Make a copy of a photo folder

- [ ] **Copy** a real photo folder with a few hundred images (ideally mixed: JPEG, PNG,
      TIFF, some with umlauts in the name, some in subfolders), for example to
      `D:\Test\Photos-Copy`. Everything that follows happens on this copy only.
- [ ] Copy it a second time, for example to `D:\Test\Photos-Copy2` (for step 3).
- [ ] Write a hash list of all JPEG originals of the copy (`*.jpg` and `*.jpeg`, also in
      subfolders) with relative paths to `D:\Test\hashes-before.csv`. PowerShell (Windows
      PowerShell 5.1 is enough), adjust the path `$source`:

  ```powershell
  function Get-HashList($folder, $extensions) {
      Push-Location -LiteralPath $folder
      try {
          Get-ChildItem -Recurse -File | Where-Object { $_.Extension -in $extensions } | ForEach-Object {
              [pscustomobject]@{
                  Path = (Resolve-Path -LiteralPath $_.FullName -Relative).Substring(2)
                  Hash = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash
              }
          }
      } finally { Pop-Location }
  }
  $source = "D:\Test\Photos-Copy"
  Get-HashList $source ".jpg", ".jpeg" | Export-Csv "D:\Test\hashes-before.csv" -NoTypeInformation -Encoding UTF8
  "{0} originals recorded" -f @(Import-Csv "D:\Test\hashes-before.csv").Count
  ```

  The count must match the number of JPEGs in the folder. The CSV is needed in step 4.

## 2. Run without Recycle Bin

- [ ] Mode "To JPEG XL", level "Balanced", "Keep metadata" on, Recycle Bin switch **off**.
- [ ] Drag the folder `Photos-Copy` into the window. The list scrolls smoothly and the count
      matches the folder.
- [ ] Start the conversion. Try "Stop" and "Resume" in between; running files finish, nothing
      new starts, and after "Resume" it continues.
- [ ] At the end: the summary shows done / skipped / errors and the sizes.
- [ ] Next to every image there is now a `.jxl`; **all originals are still there**.
- [ ] The totals match:

  ```powershell
  (Get-ChildItem D:\Test\Photos-Copy -Recurse -Filter *.jxl | Measure-Object Length -Sum).Sum
  ```

  The result equals the "after" size in the summary.
- [ ] Select an entry with "Done" and look at the before/after comparison: no visible
      difference at "Balanced". Notes and error reasons show in the tooltip when you hover over
      the status.
- [ ] There are no `*.tmp` leftovers in the folders.
- [ ] Start the run again with the same files (clear the list, drag the folder in again):
      everything is "Skipped" (existing targets are never overwritten).

## 3. Run with Recycle Bin

- [ ] On `Photos-Copy2`: Recycle Bin switch **on**, level "Lossless" (lossless results and
      losslessly repacked JPEGs are the cases where originals may go to the Recycle Bin).
- [ ] Drag the folder in, convert.
- [ ] The losslessly converted originals are in the **Windows Recycle Bin** (restorable from
      there), the `.jxl` is at the old place.
- [ ] Files that were not converted losslessly, or whose metadata did not come along
      completely, stay where they are and are "Done with note".
- [ ] Date: the `.jxl` has the same creation and modification date as the original
      (properties in Explorer or `Get-Item`).
- [ ] **If Windows ever** shows a "Delete file permanently?" window:
      answer **"No"**. It means the Recycle Bin cannot take the file. The app should never
      trigger this; please report it if it does.

## 4. Converting back with "Automatic" and byte comparison

The conversion back goes into its **own output folder**, so nothing is "Skipped" and the
comparison is unambiguous. The basis is `Photos-Copy` from step 2 (it holds the `.jxl` files)
and the CSV from step 1.

- [ ] Recycle Bin switch **off** (it is still on from step 3).
- [ ] Mode "From JPEG XL", output format "Automatic". Output folder: "Choose folder …" →
      `D:\Test\Back` (a new, empty folder).
- [ ] Drag `Photos-Copy` into the window (only the `.jxl` files are taken), convert.
      For files repacked from JPEG, the note appears that they are restored bit-exact as the
      original JPEG. The app creates the folder structure including the name of the dragged
      folder: `D:\Test\Back\Photos-Copy…`.
- [ ] Byte comparison against the CSV (relative path without extension + hash; an original
      `.jpeg` comes back as `.jpg` and still counts as equal). `Get-HashList` from step 1
      must still be defined in this PowerShell session; otherwise paste the function block
      from step 1 again:

  ```powershell
  $before = Import-Csv "D:\Test\hashes-before.csv"
  $after  = Get-HashList "D:\Test\Back\Photos-Copy" ".jpg", ".jpeg"
  $a = @{}; foreach ($e in $before) { $a[[IO.Path]::ChangeExtension($e.Path, $null).TrimEnd(".").ToLowerInvariant()] = $e.Hash }
  $b = @{}; foreach ($e in $after)  { $b[[IO.Path]::ChangeExtension($e.Path, $null).TrimEnd(".").ToLowerInvariant()] = $e.Hash }
  "Before: $($a.Count) files, after: $($b.Count) files"
  $differences = foreach ($k in ($a.Keys + $b.Keys | Sort-Object -Unique)) {
      if (-not $b.ContainsKey($k))     { "MISSING after:   $k" }
      elseif (-not $a.ContainsKey($k)) { "EXTRA after:     $k" }
      elseif ($a[$k] -ne $b[$k])       { "DIFFERENT:       $k" }
  }
  if ($differences) { $differences } else { "all equal" }
  ```

  Expected: the same count on both sides and the output `all equal`. Any other line
  (`MISSING`, `EXTRA`, `DIFFERENT`) is an error that must be reported. The number after
  "Before" must also match the count recorded in step 1; if it is smaller, a folder holds
  two JPEGs with the same name and a different extension (for example `photo.jpg` and
  `photo.jpeg`), of which only one was converted (the other is "Skipped").
- [ ] PNG/TIFF, converted losslessly and back: the images look the same and have the same
      pixel dimensions (the bytes may differ, the pixels may not).

## 5. USB stick test (no Recycle Bin)

- [ ] Put a copy of a few photos on a USB stick.
- [ ] Recycle Bin switch **on**, drag the folder from the stick in, convert.
- [ ] The originals **stay on the stick**, the `.jxl` is next to them.
- [ ] The entries are "Done with note" with the text "Original kept: no Recycle Bin on this
      drive".
- [ ] Nothing was deleted permanently; the stick holds originals and `.jxl` files.

## 6. Portable from the USB stick

The app folder `JPEG XL Converter\` (with the three `.exe` files and `LICENSES\`, **without**
`data\`) sits on a USB stick, and the check runs from there.

- [ ] Beforehand, note which folders exist under `%APPDATA%` and `%LOCALAPPDATA%` (for
      example write down the names in both folders or save the folder listings).
- [ ] Start `JPEG XL Converter.exe` **from the stick**. `data\` is created next to the exe.
- [ ] In the app settings (sliders button in the title bar), set **Language** to "Deutsch" and
      **Appearance** to the respective other setting (Light ↔ Dark). The whole interface
      switches language at once, including the window title.
- [ ] **Close** the window with the close button. It closes and the process ends.
      `%TEMP%\jpeg-xl-konverter\<pid>` including `webview` is gone afterwards.
- [ ] **Start the app from the stick again:** language and appearance are still what you set
      (the settings live in `data\settings.json`, the window position in `data\window.json`).
- [ ] Do a short run. The log is in `data\logs\` on the stick; "Open log" opens it.
- [ ] **No app folder was created under `%APPDATA%` or `%LOCALAPPDATA%`** (no
      `de.bassboost1981.jpegxlkonverter`, no `JPEG XL Konverter`, no `JPEG XL Converter`).
      The listings match the note from above. Folders of an earlier version may be there; the
      app neither picks them up nor deletes them (you can delete them by hand).
- [ ] Optional, read-only mode: lock the stick with its write-protect switch (or put the app
      folder in a place without write permission) and start the app. An existing
      `data\settings.json` is read (language and appearance are right), once per start the notice
      "App folder is read-only – settings are not saved, logs go to the temp folder." appears,
      changes take effect but are not saved, and the log is in `%TEMP%\jpeg-xl-konverter\logs\`.

## 7. No network access

- [ ] Open Resource Monitor (`resmon`), "Network" tab.
- [ ] Start the app and do a run. Under "Processes with Network Activity" and
      "TCP Connections", neither `JPEG XL Converter.exe` nor `cjxl.exe`/`djxl.exe` shows up.
- [ ] Optional: disconnect the network (airplane mode); everything works and the font is Lexend.

## 8. Finally: the real archive

Only when **all** checkboxes above are ticked:

- [ ] A backup of the archive exists (a real, separate copy, for example on an external drive).
- [ ] First convert a single real subfolder and check the result.
- [ ] Then the whole archive, first without the Recycle Bin, afterwards (if everything fits)
      with the Recycle Bin.
- [ ] Do not empty the Recycle Bin right away; only after a few days, once the `.jxl` files
      have proven themselves in everyday use.

## Known limitations

- No EXR files, no CMYK JPEGs and no floating-point images (HDR/float): they are rejected or
  flagged with an error/note and left untouched.
- Multi-page TIFFs and animations (animated GIFs): the first page or first frame is checked,
  so **the originals always stay**, even with the Recycle Bin switch on.
- Converting back floating-point JPEG XL (HDR), images with more than 16 bits or with extra
  channels other than alpha: PNG/TIFF store at most 16-bit integers, values above 1.0 and
  extra channels are lost. The `.jxl` therefore **always stays** (note "Floating-point or
  multi-channel image – .jxl kept").
- "Keep metadata" off: the EXIF orientation is discarded too. Images that were only shown
  rotated through that value may appear rotated afterwards.
- Windows 11 (x64) only. Thumbnails for `.jxl` in Explorer come from Windows only with
  Microsoft's "JPEG XL Image Extension".
