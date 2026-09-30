import type { ReactNode } from 'react';
import { S } from '../../strings/catalogue';

export function AuthLayout({
  title,
  children,
  footer,
}: {
  title: string;
  children: ReactNode;
  footer?: ReactNode;
}) {
  return (
    <div className="app-auth">
      <main className="app-auth__card p-strip">
        <h1 className="p-heading--2">{title}</h1>
        {children}
        {footer ? <div className="app-auth__footer">{footer}</div> : null}
      </main>
    </div>
  );
}

export function AuthSeparator({ label }: { label: string }) {
  return (
    <div className="app-auth__separator">
      <span className="app-auth__separator-line" />
      <span className="app-auth__separator-label">{label}</span>
      <span className="app-auth__separator-line" />
    </div>
  );
}

export function AppMark() {
  return <p className="app-auth__mark">{S.app.name}</p>;
}
