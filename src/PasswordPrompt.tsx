// Asks for the password of an encrypted PDF (FR-03). The password is only passed to open_pdf, never kept.
import { useState } from "react";

interface Props {
  fileName: string;
  message: string;
  busy: boolean;
  onSubmit: (password: string) => void;
  onCancel: () => void;
}

export function PasswordPrompt({ fileName, message, busy, onSubmit, onCancel }: Props) {
  const [password, setPassword] = useState("");

  return (
    <form
      className="password-prompt"
      aria-label={`Password for ${fileName}`}
      onSubmit={(e) => {
        e.preventDefault();
        onSubmit(password);
        setPassword("");
      }}
      onKeyDown={(e) => e.key === "Escape" && onCancel()}
    >
      <strong>{fileName}</strong>
      <p role="alert">{message}</p>
      <label>
        Password
        <input
          type="password"
          autoFocus
          autoComplete="off"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
        />
      </label>
      <div className="row">
        <button type="submit" className="primary" disabled={busy || !password}>
          Open
        </button>
        <button type="button" onClick={onCancel}>
          Cancel
        </button>
      </div>
    </form>
  );
}
