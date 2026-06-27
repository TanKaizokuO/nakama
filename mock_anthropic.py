import http.server
import socketserver
import json
import time

class Handler(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        content_length = int(self.headers.get('Content-Length', 0))
        post_data = self.rfile.read(content_length)
        req = json.loads(post_data)
        
        self.send_response(200)
        self.send_header('Content-type', 'text/event-stream')
        self.end_headers()
        
        messages = req.get("messages", [])
        last_msg = messages[-1]
        
        if last_msg["role"] == "user" and isinstance(last_msg["content"], list) and last_msg["content"][0].get("type") == "tool_result":
            # This is the tool result, return final text
            events = [
                'event: message_start\ndata: {"type": "message_start", "message": {"id": "msg_2", "role": "assistant", "model": "claude", "usage": {"input_tokens": 50, "output_tokens": 0}}}\n\n',
                'event: content_block_start\ndata: {"type": "content_block_start", "index": 0, "content_block": {"type": "text", "text": ""}}\n\n',
                'event: content_block_delta\ndata: {"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": "Tool executed successfully!"}}\n\n',
                'event: content_block_stop\ndata: {"type": "content_block_stop", "index": 0}\n\n',
                'event: message_delta\ndata: {"type": "message_delta", "delta": {"stop_reason": "end_turn"}, "usage": {"output_tokens": 5}}\n\n',
                'event: message_stop\ndata: {"type": "message_stop"}\n\n'
            ]
        else:
            # Initial prompt, return tool call
            events = [
                'event: message_start\ndata: {"type": "message_start", "message": {"id": "msg_1", "role": "assistant", "model": "claude", "usage": {"input_tokens": 15, "output_tokens": 0}}}\n\n',
                'event: content_block_start\ndata: {"type": "content_block_start", "index": 0, "content_block": {"type": "text", "text": ""}}\n\n',
                'event: content_block_delta\ndata: {"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": "I will execute a tool now. "}}\n\n',
                'event: content_block_stop\ndata: {"type": "content_block_stop", "index": 0}\n\n',
                'event: content_block_start\ndata: {"type": "content_block_start", "index": 1, "content_block": {"type": "tool_use", "id": "tool_123", "name": "bash", "input": {}}}\n\n',
                'event: content_block_delta\ndata: {"type": "content_block_delta", "index": 1, "delta": {"type": "input_json_delta", "partial_json": "{\\"co"}}\n\n',
                'event: content_block_delta\ndata: {"type": "content_block_delta", "index": 1, "delta": {"type": "input_json_delta", "partial_json": "mmand\\": \\"echo verified\\"}"}}\n\n',
                'event: content_block_stop\ndata: {"type": "content_block_stop", "index": 1}\n\n',
                'event: message_delta\ndata: {"type": "message_delta", "delta": {"stop_reason": "tool_use"}, "usage": {"output_tokens": 10}}\n\n',
                'event: message_stop\ndata: {"type": "message_stop"}\n\n'
            ]
            
        for e in events:
            try:
                self.wfile.write(e.encode('utf-8'))
                self.wfile.flush()
                time.sleep(0.1)
            except Exception:
                break

with socketserver.TCPServer(("127.0.0.1", 8080), Handler) as httpd:
    httpd.serve_forever()
