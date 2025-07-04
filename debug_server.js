// Simple debug HTTP server
// Usage: node debug_server.js

const http = require('http');
const port = 8080;

// Request counter
let requestCount = 0;

// Create HTTP server
const server = http.createServer((req, res) => {
  const requestId = requestCount++;
  const reqTime = new Date().toISOString();
  
  // Log request details
  console.log(`[${reqTime}] Request #${requestId}: ${req.method} ${req.url}`);
  console.log(`Headers: ${JSON.stringify(req.headers, null, 2)}`);
  
  // Send response with request info
  res.writeHead(200, { 'Content-Type': 'application/json' });
  res.end(JSON.stringify({
    message: 'Debug server response',
    requestId: requestId,
    time: reqTime,
    method: req.method,
    url: req.url,
    headers: req.headers
  }));
});

// Start server
server.listen(port, () => {
  console.log(`Debug server running at http://localhost:${port}/`);
  console.log('Use Ctrl+C to stop');
});