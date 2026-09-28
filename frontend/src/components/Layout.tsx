import { ReactNode } from 'react';
import { useAuthStore } from '../stores/auth';

interface LayoutProps {
  children: ReactNode;
}

export function Layout({ children }: LayoutProps) {
  const { user, logout } = useAuthStore();

  return (
    <div className="l-application">
      <header className="p-panel l-application__header">
        <div className="p-panel__header">
          <h1 className="p-heading--4">Menzi</h1>
        </div>
        <nav className="p-panel__content">
          <ul className="p-list">
            <li className="p-list__item"><a href="/projects">Projects</a></li>
            <li className="p-list__item"><a href="/design">Design</a></li>
            <li className="p-list__item"><a href="/settings">Settings</a></li>
          </ul>
        </nav>
        {user && (
          <div className="p-panel__footer">
            <span>{user.name}</span>
            <button className="p-button--link" onClick={logout}>Sign out</button>
          </div>
        )}
      </header>
      <main className="l-application__main">
        {children}
      </main>
    </div>
  );
}
