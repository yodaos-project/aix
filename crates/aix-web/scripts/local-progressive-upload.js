const { createHash, randomUUID } = require('node:crypto');
const fs = require('node:fs/promises');
const path = require('node:path');

const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const validDigest = value => typeof value === 'string' && /^[0-9a-f]{64}$/.test(value);
const objectLimit = 32 * 1024 * 1024;

async function readBody(request, limit) {
  const chunks = [];
  let size = 0;
  for await (const chunk of request) {
    size += chunk.length;
    if (size > limit) throw new Error('Request exceeds upload budget');
    chunks.push(chunk);
  }
  return Buffer.concat(chunks, size);
}

function json(response, status, value) {
  response.writeHead(status, { 'Content-Type': 'application/json', 'Cache-Control': 'no-store' });
  response.end(JSON.stringify(value));
}

// Local Studio/CDN stand-in. Receipts are emitted only after the complete
// inventory has been checked against the bytes actually stored by the server.
function localProgressiveUpload(directory) {
  const sessions = new Map();
  const requests = [];
  const invalid = new Set();
  const advertised = new Set();
  const handle = async (request, response, pathname, origin) => {
    if (!pathname.startsWith('/studio/') && !pathname.startsWith('/objects/')) return false;
    requests.push([request.method, pathname]);
    try {
      if (request.method === 'POST' && pathname === '/studio/uploads') {
        const { assetVersion, objects } = JSON.parse(await readBody(request, 4 * 1024 * 1024));
        if (!validDigest(assetVersion) || !Array.isArray(objects) || !objects.length || objects.length > 8192) {
          throw new Error('Invalid upload plan');
        }
        const inventory = new Map();
        for (const { id, size, sha256 } of objects) {
          if (!validDigest(id) || id !== sha256 || !Number.isSafeInteger(size)
            || size <= 0 || size > objectLimit || inventory.has(id)) {
            throw new Error('Invalid object inventory');
          }
          inventory.set(id, { id, size, sha256 });
        }
        const uploadId = randomUUID();
        sessions.set(uploadId, { assetVersion, inventory, stored: new Set() });
        json(response, 201, { uploadId });
      } else if (request.method === 'PUT' && /^\/studio\/uploads\/[^/]+\/[^/]+$/.test(pathname)) {
        const [, , , uploadId, id] = pathname.split('/');
        const session = sessions.get(uploadId);
        const expected = session?.inventory.get(id);
        if (!expected) throw new Error('Unexpected object');
        const bytes = await readBody(request, expected.size);
        if (bytes.length !== expected.size || digest(bytes) !== id) throw new Error('Object integrity mismatch');
        const temporary = path.join(directory, randomUUID());
        try {
          await fs.writeFile(temporary, bytes, { flag: 'wx', flush: true });
          await fs.rename(temporary, path.join(directory, id));
          session.stored.add(id);
        } finally {
          await fs.rm(temporary, { force: true });
        }
        response.writeHead(204).end();
      } else if (request.method === 'POST' && /^\/studio\/uploads\/[^/]+\/complete$/.test(pathname)) {
        const [, , , uploadId] = pathname.split('/');
        const session = sessions.get(uploadId);
        if (!session || session.stored.size !== session.inventory.size) throw new Error('Incomplete upload');
        const objects = [];
        for (const object of session.inventory.values()) {
          const stored = await fs.readFile(path.join(directory, object.id));
          if (stored.length !== object.size || digest(stored) !== object.sha256) throw new Error('Corrupt upload');
          objects.push({ ...object, url: origin + '/objects/' + object.id });
        }
        for (const object of objects) advertised.add(object.id);
        json(response, 200, { uploadId, assetVersion: session.assetVersion, objects });
        sessions.delete(uploadId);
      } else if (request.method === 'GET' && validDigest(pathname.slice('/objects/'.length))
        && pathname.startsWith('/objects/')) {
        const id = pathname.slice('/objects/'.length);
        if (!advertised.has(id)) {
          response.writeHead(404).end();
          return true;
        }
        const bytes = await fs.readFile(path.join(directory, id));
        if (invalid.has(id)) bytes[0] ^= 1;
        response.writeHead(200, {
          'Content-Type': 'application/octet-stream',
          'Content-Length': bytes.length,
          'Cache-Control': 'public, max-age=31536000, immutable',
        });
        response.end(bytes);
      } else {
        response.writeHead(404).end();
      }
    } catch (error) {
      if (!response.headersSent) json(response, 400, { error: error.message });
      else response.destroy();
    }
    return true;
  };
  return { handle, requests, invalid };
}

module.exports = { localProgressiveUpload };
