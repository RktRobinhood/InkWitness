# Manual test: does Microsoft Word keep the InkWitness evidence part?

**Status: NOT TESTED.** No agent has Word. This is the procedure for a person with Word desktop on Windows (TEST_MATRIX D03). The same steps work for Word for Mac (D04) and Word for the web (D05); record which one you used.

It takes about 10 minutes. You need Word and a way to look inside a ZIP file. Windows File Explorer can do this once you rename the file to `.zip`. 7-Zip works without renaming.

## 1. Get the test file

Download the synthetic fixture produced by the real WordCraft writer:

- In the repository: `vendor/wordcraft/crates/inkwitness/tests/fixtures/g0-evidence.docx`
- On GitHub: open that path on the `main` branch and click **Download raw file**.

It holds one paragraph ("Example Student wrote this. A synthetic pasted sentence.") and a Custom XML part with a small synthetic event log. It contains no real student data.

Make two copies in a new folder:

- `g0-evidence-original.docx`: never open this one.
- `g0-evidence-word.docx`: the one you edit.

## 2. Write down the setup

- Word version: **File → Account → About Word** (for example "Microsoft® Word for Microsoft 365 MSO (Version 2409 Build 16.0.18025.20160) 64-bit").
- Windows or macOS version.
- Whether the file came from the internet zone (Protected View banner).

## 3. Open, edit and save in Word

1. Open `g0-evidence-word.docx` in Word.
2. If a yellow **Protected View** bar appears, click **Enable Editing**. Note it.
3. If Word says the file is corrupt or offers to repair it, **stop and record the exact message**. That is a failed test.
4. Click at the end of the paragraph and type: ` Word edit by Example Student.`
5. Press **Enter** and type a second line: `A second paragraph typed in Word.`
6. Save with **Ctrl+S** (Mac: **Cmd+S**). If Word asks about the format or compatibility mode, keep **Word Document (.docx)**.
7. Close Word completely.

## 4. Look inside the saved file

1. Copy `g0-evidence-word.docx` and rename the copy to `g0-evidence-word.zip`. Windows may ask whether to change the extension; answer yes. Or skip renaming and open the `.docx` with 7-Zip.
2. Open the ZIP and check each item below. Write down what you see.

| Check | Where | Pass when |
|---|---|---|
| A. The folder exists | `customXml/` at the top level | It is there |
| B. The evidence part | Open each `customXml/item*.xml` in Notepad (not `itemProps*`) | One of them starts with `<wa:history xmlns:wa="urn:workalone:evidence:v1"` (Word may use a different prefix than `wa`), has `attemptId="00000000-0000-4000-8000-000000000001"`, and holds one long run of letters and digits (the base64 payload) |
| C. Its properties part | The matching `customXml/itemProps*.xml` | It has `ds:itemID="{1A2B3C4D-0000-4000-8000-000000000001}"` and `ds:schemaRef ds:uri="urn:workalone:evidence:v1"` |
| D. It is still related | `word/_rels/document.xml.rels` | One `Relationship` has `Type` ending in `/customXml` and a `Target` naming the file from B |
| E. Your edit was saved | `word/document.xml` | It contains `Word edit by Example Student.` |

Word may rename the part (for example `item1.xml` becomes `item2.xml`), reorder the ZIP, or rewrite the XML declaration. That is fine: InkWitness finds the part by its namespace, not its file name.

## 5. Report the result

Put these on the Paperclip task (or in a GitHub issue):

- Word version and operating system (step 2).
- Any Protected View or repair prompt (step 3).
- A to E: pass or fail, and for a failure what you saw instead.
- **Attach `g0-evidence-word.docx`.** An agent then runs the real reader on it. This confirms the payload decodes byte-for-byte, which a visual check can't do:

  ```sh
  cd vendor/wordcraft
  cargo run -p wordcraft-inkwitness --example evidence_check -- g0-evidence-word.docx
  ```

  The fixture itself gives `evidence found in customXml/item1.xml (item id {1A2B3C4D-0000-4000-8000-000000000001}): attempt 00000000-0000-4000-8000-000000000001, … payload bytes, … events, payload check …`. A Word-saved file passes when the attempt, payload byte count, event count and payload check are the same.

Record the outcome in `docs/BACKLOG.md` (G0, "Microsoft Word Windows DOCX edit/save preservation test") with the date and Word version. If Word drops the part, that is a valid result too: record it as FAIL with the version. The reviewer must then show "history missing", not invent history (TEST_MATRIX D09).
