import { useEffect, useMemo, useState } from 'react';
import { WORKING_INTERVAL_MS, workingSequence } from '../../strings/working';
import { S } from '../../strings/catalogue';

export function WorkingLabel() {
  const [sequence, setSequence] = useState(workingSequence);
  const [index, setIndex] = useState(0);

  useEffect(() => {
    const timer = setInterval(() => {
      setIndex((current) => {
        const next = current + 1;
        if (next < sequence.length) return next;
        setSequence(workingSequence());
        return 0;
      });
    }, WORKING_INTERVAL_MS);
    return () => clearInterval(timer);
  }, [sequence.length]);

  const phrase = useMemo(() => sequence[index % sequence.length], [sequence, index]);

  return (
    <span className="app-working" role="status" data-testid="working-label">
      <span className="app-working__dot" aria-hidden="true" />
      <span className="app-working__text" key={phrase.language} lang={phrase.language}>
        {phrase.text}
      </span>
      <span className="u-off-screen">{S.chat.working}</span>
    </span>
  );
}