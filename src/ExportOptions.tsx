// File names and ZIP choice for the export, shown on the review screen (FR-51 to FR-53).
import { namePreview } from "./naming";

interface Props {
  template: string;
  onTemplate: (template: string) => void;
  /** The names the template gives the parts, from the backend, without ".pdf". */
  names: string[];
  zip: boolean;
  onZip: (zip: boolean) => void;
  /** The ZIP's name, without ".zip". */
  zipName: string;
}

export function ExportOptions({ template, onTemplate, names, zip, onZip, zipName }: Props) {
  const preview = namePreview(names);
  return (
    <fieldset className="export-options">
      <legend>File names</legend>
      <label>
        Template
        <input
          value={template}
          onChange={(e) => onTemplate(e.target.value)}
          aria-describedby="name-tokens name-preview"
          spellCheck={false}
        />
      </label>
      <p id="name-tokens" className="muted">
        <code>{"{index}"}</code> part number, <code>{"{title}"}</code> part title, <code>{"{original}"}</code> PDF name
      </p>
      <ul id="name-preview" className="name-preview" aria-label="File names">
        {preview.shown.map((name, i) => (
          <li key={i}>{name}.pdf</li>
        ))}
        {preview.more > 0 && (
          <li className="muted">
            and {preview.more} more
          </li>
        )}
      </ul>
      <label className="check">
        <input type="checkbox" checked={zip} onChange={(e) => onZip(e.target.checked)} />
        Bundle into one ZIP file ({zipName}.zip)
      </label>
    </fieldset>
  );
}
