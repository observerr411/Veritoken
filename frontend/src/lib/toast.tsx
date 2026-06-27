import {
  createContext,
  useContext,
  useState,
  useEffect,
  useCallback,
  type ReactNode,
} from "react";

interface Toast {
  id: number;
  type: "success" | "error" | "info";
  message: string;
}

interface ToastContextValue {
  addToast: (message: string, type?: Toast["type"]) => void;
}

const ToastContext = createContext<ToastContextValue>({ addToast: () => {} });

let nextId = 0;

const COLOR: Record<Toast["type"], { bg: string; border: string; text: string }> = {
  success: { bg: "#dcfce7", border: "#4ade80", text: "#166534" },
  error:   { bg: "#fee2e2", border: "#f87171", text: "#991b1b" },
  info:    { bg: "#dbeafe", border: "#60a5fa", text: "#1e40af" },
};

function ToastItem({ toast, onRemove }: { toast: Toast; onRemove: (id: number) => void }) {
  useEffect(() => {
    const t = setTimeout(() => onRemove(toast.id), 5000);
    return () => clearTimeout(t);
  }, [toast.id, onRemove]);

  const c = COLOR[toast.type];
  return (
    <div
      role="status"
      style={{
        display: "flex",
        justifyContent: "space-between",
        alignItems: "flex-start",
        gap: "0.75rem",
        padding: "0.75rem 1rem",
        borderRadius: 10,
        background: c.bg,
        border: `1px solid ${c.border}`,
        color: c.text,
        fontSize: "0.875rem",
        boxShadow: "0 4px 12px rgba(0,0,0,0.12)",
        minWidth: 260,
        maxWidth: 380,
      }}
    >
      <span>{toast.message}</span>
      <button
        aria-label="Dismiss"
        onClick={() => onRemove(toast.id)}
        style={{ background: "none", border: "none", cursor: "pointer", padding: 0, color: "inherit", lineHeight: 1 }}
      >
        ✕
      </button>
    </div>
  );
}

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);

  const remove = useCallback((id: number) => {
    setToasts((t) => t.filter((x) => x.id !== id));
  }, []);

  const addToast = useCallback((message: string, type: Toast["type"] = "info") => {
    setToasts((t) => [...t, { id: nextId++, type, message }]);
  }, []);

  return (
    <ToastContext.Provider value={{ addToast }}>
      {children}
      <div
        style={{
          position: "fixed",
          bottom: "1.5rem",
          right: "1.5rem",
          display: "flex",
          flexDirection: "column",
          gap: "0.5rem",
          zIndex: 9999,
        }}
      >
        {toasts.map((t) => (
          <ToastItem key={t.id} toast={t} onRemove={remove} />
        ))}
      </div>
    </ToastContext.Provider>
  );
}

export function useToast() {
  return useContext(ToastContext);
}
