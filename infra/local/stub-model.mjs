import { createServer } from 'node:http';

const PORT = Number(process.env.MENZI_STUB_MODEL_PORT || 18000);
const REPLY = process.env.MENZI_STUB_REPLY || 'MENZI-OK';
const CHUNKS = [
  'Reading the request. ',
  'The pipeline is live: ',
  'this reply came from the local stub model via opencode.',
];

function sendJson(res, status, body) {
  const payload = JSON.stringify(body);
  res.writeHead(status, { 'content-type': 'application/json' });
  res.end(payload);
}

const server = createServer((req, res) => {
  const url = req.url ?? '';

  if (req.method === 'GET' && url.startsWith('/v1/models')) {
    sendJson(res, 200, {
      object: 'list',
      data: [{ id: 'mini', object: 'model', owned_by: 'local-stub' }],
    });
    return;
  }

  if (req.method === 'POST' && url.startsWith('/v1/chat/completions')) {
    let body = '';
    req.on('data', (chunk) => {
      body += chunk;
    });
    req.on('end', () => {
      let wantsStream = true;
      try {
        wantsStream = JSON.parse(body || '{}').stream !== false;
      } catch {
        wantsStream = true;
      }

      const id = `chatcmpl-stub-${Date.now()}`;
      const created = Math.floor(Date.now() / 1000);

      if (!wantsStream) {
        sendJson(res, 200, {
          id,
          object: 'chat.completion',
          created,
          model: 'mini',
          choices: [
            {
              index: 0,
              message: { role: 'assistant', content: `${REPLY} ${CHUNKS.join('')}` },
              finish_reason: 'stop',
            },
          ],
          usage: { prompt_tokens: 8, completion_tokens: 12, total_tokens: 20 },
        });
        return;
      }

      res.writeHead(200, {
        'content-type': 'text/event-stream',
        'cache-control': 'no-cache',
        connection: 'keep-alive',
      });

      const send = (delta, finish = null) => {
        res.write(
          `data: ${JSON.stringify({
            id,
            object: 'chat.completion.chunk',
            created,
            model: 'mini',
            choices: [{ index: 0, delta, finish_reason: finish }],
          })}\n\n`,
        );
      };

      send({ role: 'assistant', content: '' });
      let index = 0;
      const timer = setInterval(() => {
        if (index < CHUNKS.length) {
          send({ content: CHUNKS[index] });
          index += 1;
          return;
        }
        clearInterval(timer);
        send({}, 'stop');
        res.write('data: [DONE]\n\n');
        res.end();
      }, 40);
    });
    return;
  }

  sendJson(res, 404, { error: 'not found' });
});

server.listen(PORT, '127.0.0.1', () => {
  console.log(`stub model listening on http://127.0.0.1:${PORT}/v1`);
});
