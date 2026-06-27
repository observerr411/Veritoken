import { useState, useRef, useEffect } from "react";
import { useAddressBook } from "../lib/addressBook";

export function AddressInput({
  label,
  value,
  onChange,
  placeholder,
  required,
  onAddToBook,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  required?: boolean;
  onAddToBook?: (address: string) => void;
}) {
  const { search } = useAddressBook();
  const [suggestions, setSuggestions] = useState<Array<{ address: string; label: string }>>([]);
  const [showSuggestions, setShowSuggestions] = useState(false);
  const suggestionsRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (value && value.length > 0) {
      const results = search(value);
      setSuggestions(results);
      setShowSuggestions(results.length > 0);
    } else {
      setSuggestions([]);
      setShowSuggestions(false);
    }
  }, [value, search]);

  useEffect(() => {
    function handleClickOutside(e: MouseEvent) {
      if (suggestionsRef.current && !suggestionsRef.current.contains(e.target as Node)) {
        setShowSuggestions(false);
      }
    }
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

  return (
    <div style={{ position: "relative" }}>
      <div className="field">
        <label>{label}</label>
        <input
          type="text"
          value={value}
          onChange={(e) => onChange(e.target.value)}
          onFocus={() => suggestions.length > 0 && setShowSuggestions(true)}
          placeholder={placeholder}
          required={required}
        />
      </div>

      {showSuggestions && suggestions.length > 0 && (
        <div
          ref={suggestionsRef}
          style={{
            position: "absolute",
            top: "calc(100% - 0.5rem)",
            left: 0,
            right: 0,
            background: "var(--surface-2)",
            border: "1px solid var(--border)",
            borderRadius: 8,
            maxHeight: 200,
            overflowY: "auto",
            zIndex: 10,
          }}
        >
          {suggestions.map((s) => (
            <div
              key={s.address}
              onClick={() => {
                onChange(s.address);
                setShowSuggestions(false);
              }}
              style={{
                padding: "0.75rem 1rem",
                cursor: "pointer",
                borderBottom: "1px solid var(--border)",
                fontSize: "0.9rem",
              }}
              onMouseEnter={(e) => {
                (e.currentTarget as HTMLElement).style.background = "var(--surface-3)";
              }}
              onMouseLeave={(e) => {
                (e.currentTarget as HTMLElement).style.background = "transparent";
              }}
            >
              <div style={{ fontWeight: 600 }}>{s.label}</div>
              <div className="mono muted" style={{ fontSize: "0.8rem", marginTop: "0.2rem" }}>
                {s.address.slice(0, 6)}…{s.address.slice(-6)}
              </div>
            </div>
          ))}
        </div>
      )}

      {onAddToBook && value && value.length > 20 && (
        <button
          type="button"
          onClick={() => onAddToBook(value)}
          style={{
            marginTop: "0.5rem",
            fontSize: "0.8rem",
            padding: "0.35rem 0.7rem",
          }}
          className="btn-ghost"
        >
          + Add to Address Book
        </button>
      )}
    </div>
  );
}
