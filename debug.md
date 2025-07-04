# Debugging HTTP Request Statistics

This document provides instructions for diagnosing issues with HTTP request recording in the Blamo Web Throughput tool.

## Step 1: Run the Debug Server

First, start the debug HTTP server that will log all incoming requests:

```
node debug_server.js
```

This server runs on port 8080 and outputs detailed information about each request it receives.

## Step 2: Run the Debug Helper

Run the standalone debug helper that uses direct HTTP requests:

```
cargo run --bin debug_helper http://localhost:8080 -n 10 -c 5
```

This will send 10 requests with 5 concurrent connections to the debug server and display detailed information about each request and response.

## Step 3: Run Blamo in Debug Mode

Run the main application with the `--debug` flag to use the simplified request handler:

```
cargo run -- http://localhost:8080 -n 10 -c 5 --debug
```

This bypasses the UI and sophisticated task management to provide direct output about each request.

## Step 4: Compare Results

Compare the output from all three sources:

1. Debug server logs: How many requests were actually received?
2. Debug helper: How many requests were reported as successful?
3. Blamo debug mode: How many requests were recorded?

## Potential Issues and Solutions

### Requests Not Being Made

If the debug server doesn't show requests but the debug helper/tool reports sending them:
- Check for network connectivity issues
- Verify URL formatting is correct
- Check if a firewall is blocking connections

### Requests Being Made But Not Recorded

If the debug server shows requests but the stats don't appear in Blamo:
- The message passing between tasks might be failing
- The state update logic might be dropping updates
- The rendering code might not be correctly displaying the stats

### Channel Issues

If messages are being sent but not received:
- Verify channels are not being dropped prematurely
- Check that senders and receivers are properly connected
- Make sure there are no deadlocks in the async tasks

## Testing for Specific Status Codes

To test handling of different HTTP statuses, modify the debug server to return specific status codes:

```javascript
// For 404 responses
if (req.url === '/not-found') {
  res.writeHead(404, { 'Content-Type': 'text/plain' });
  res.end('Not Found');
  return;
}

// For 500 responses
if (req.url === '/error') {
  res.writeHead(500, { 'Content-Type': 'text/plain' });
  res.end('Server Error');
  return;
}
```

Then test with:
```
cargo run -- http://localhost:8080/not-found -n 5 -c 2 --debug
cargo run -- http://localhost:8080/error -n 5 -c 2 --debug
```

This helps verify that different HTTP status codes are being correctly recorded.