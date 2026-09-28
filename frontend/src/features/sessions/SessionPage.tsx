import { useEffect, useState } from 'react';
import { useParams } from 'react-router-dom';
import { getSession, getMessages, sendPrompt } from './api';
import type { Session, Message } from '../../lib/types';

export function SessionPage() {
  const { sessionId } = useParams<{ sessionId: string }>();
  const [session, setSession] = useState<Session | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [prompt, setPrompt] = useState('');
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    if (!sessionId) return;
    Promise.all([getSession(sessionId), getMessages(sessionId)])
      .then(([s, m]) => {
        setSession(s);
        setMessages(m);
      })
      .catch(console.error)
      .finally(() => setLoading(false));
  }, [sessionId]);

  const handleSend = async () => {
    if (!sessionId || !prompt.trim()) return;
    await sendPrompt(sessionId, prompt);
    setPrompt('');
  };

  if (loading) {
    return <p>Loading session...</p>;
  }

  return (
    <div className="l-equal-grid">
      <div className="l-equal-grid__col">
        <div className="p-card">
          <div className="p-card__header">
            <h3 className="p-heading--5">{session?.name}</h3>
          </div>
          <div className="p-card__content">
            <p>Status: {session?.status}</p>
            {session?.branch && <p>Branch: {session.branch}</p>}
          </div>
        </div>
      </div>
      <div className="l-equal-grid__col">
        <div className="p-card">
          <div className="p-card__header">
            <h3 className="p-heading--5">Conversation</h3>
          </div>
          <div className="p-card__content">
            {messages.map((msg) => (
              <div key={msg.id} className="p-card--highlighted">
                <p><strong>{msg.role}</strong></p>
                <p>{msg.content}</p>
              </div>
            ))}
          </div>
          <div className="p-card__footer">
            <input
              type="text"
              value={prompt}
              onChange={(e) => setPrompt(e.target.value)}
              placeholder="Type a message..."
              className="p-form__control"
            />
            <button className="p-button--positive" onClick={handleSend}>
              Send
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
