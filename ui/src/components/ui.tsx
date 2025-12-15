import React from "react";

export function Card({
  title,
  children,
  actions
}: {
  title: string;
  children: React.ReactNode;
  actions?: React.ReactNode;
}) {
  return (
    <section className="card">
      <div className="row" style={{ justifyContent: "space-between" }}>
        <h3 className="card-title">{title}</h3>
        {actions ? <div className="row">{actions}</div> : null}
      </div>
      <div className="card-body">{children}</div>
    </section>
  );
}

export function Button({
  children,
  variant = "default",
  disabled,
  onClick,
  title,
  type
}: {
  children: React.ReactNode;
  variant?: "default" | "primary" | "danger" | "ghost";
  disabled?: boolean;
  onClick?: () => void;
  title?: string;
  type?: "button" | "submit";
}) {
  const className =
    variant === "primary"
      ? "btn btn-primary"
      : variant === "danger"
        ? "btn btn-danger"
        : variant === "ghost"
          ? "btn btn-ghost"
          : "btn";
  return (
    <button className={className} type={type ?? "button"} disabled={disabled} onClick={onClick} title={title}>
      {children}
    </button>
  );
}

export function Badge({ kind, children }: { kind?: "ok" | "bad"; children: React.ReactNode }) {
  const className = kind === "ok" ? "badge badge-ok" : kind === "bad" ? "badge badge-bad" : "badge";
  return <span className={className}>{children}</span>;
}

export function Modal({
  open,
  title,
  onClose,
  children
}: {
  open: boolean;
  title: string;
  onClose: () => void;
  children: React.ReactNode;
}) {
  if (!open) return null;
  return (
    <div
      className="modal-overlay"
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="modal">
        <div className="modal-header">
          <h3 className="modal-title">{title}</h3>
          <Button variant="ghost" onClick={onClose} title="关闭">
            关闭
          </Button>
        </div>
        <div className="modal-body">{children}</div>
      </div>
    </div>
  );
}
