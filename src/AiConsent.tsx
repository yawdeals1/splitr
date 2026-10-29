// The notice shown before the first AI use (FR-61). It says exactly what text leaves the computer.
import { useEffect, useRef } from "react";
import type { AiService } from "./ai";

interface Props {
  /** The service the excerpts go to. */
  service: AiService;
  onAgree: () => void;
  onCancel: () => void;
}

export function AiConsent({ service, onAgree, onCancel }: Props) {
  const dialog = useRef<HTMLDialogElement>(null);
  // A modal <dialog> keeps focus inside and closes on Escape (NFR-11).
  useEffect(() => {
    dialog.current?.showModal();
  }, []);

  return (
    <dialog ref={dialog} className="ai-consent" aria-labelledby="ai-consent-title" onCancel={onCancel}>
      <h2 id="ai-consent-title">Before Splitr uses AI</h2>
      <p>To find where each part starts, Splitr sends {service.name} ({service.company}), using your API key:</p>
      <ul>
        <li>the first and last few lines of text on each page, with its page number</li>
        <li>the preset or custom instruction you chose</li>
      </ul>
      <p>
        It does not send the PDF itself, page images, the file name or anything else on your computer. Large
        files are sent in several parts. Splitting by rules, Custom and Fixed modes never send anything.
      </p>
      <p className="muted">
        {service.company} handles this text under its API terms. You can turn AI off at any time in the AI section of
        the panel.
      </p>
      <div className="row">
        <button type="button" className="primary" onClick={onAgree} autoFocus>
          Agree and continue
        </button>
        <button type="button" onClick={onCancel}>
          Cancel
        </button>
      </div>
    </dialog>
  );
}
