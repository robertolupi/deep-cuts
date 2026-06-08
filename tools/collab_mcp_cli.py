import sys
import os
import argparse
import json

# Add tools dir to path
tools_dir = os.path.dirname(os.path.abspath(__file__))
if tools_dir not in sys.path:
    sys.path.insert(0, tools_dir)

from collab_mcp.store import MailStore

def main():
    parser = argparse.ArgumentParser(description="Collab MCP CLI wrapper")
    parser.add_argument("--actor", default="agy", help="Actor name")
    parser.add_argument("--root", default="scratch/coordination", help="Coordination root")
    
    subparsers = parser.add_subparsers(dest="command", required=True)
    
    # Inbox command
    subparsers.add_parser("inbox")
    
    # Send command
    send_parser = subparsers.add_parser("send")
    send_parser.add_argument("--to", required=True, help="Recipient actor")
    send_parser.add_argument("--type", required=True, help="Message type")
    send_parser.add_argument("--payload", required=True, help="JSON payload string")
    send_parser.add_argument("--in-reply-to", help="Message ID replied to")
    
    # Recv command
    recv_parser = subparsers.add_parser("recv")
    recv_parser.add_argument("--type", help="Match message type")
    recv_parser.add_argument("--timeout", type=float, help="Timeout in seconds")
    
    # Try_recv command
    try_recv_parser = subparsers.add_parser("try_recv")
    try_recv_parser.add_argument("--type", help="Match message type")
    
    # Post command
    post_parser = subparsers.add_parser("post")
    post_parser.add_argument("--payload", required=True, help="JSON payload string")
    post_parser.add_argument("--type", default="task", help="Task type")
    
    # Claim command
    claim_parser = subparsers.add_parser("claim")
    claim_parser.add_argument("--ttl", type=float, help="Lease TTL")
    
    # Complete command
    complete_parser = subparsers.add_parser("complete")
    complete_parser.add_argument("--task-id", required=True, help="Task ID")
    complete_parser.add_argument("--result", required=True, help="JSON result string")
    
    args = parser.parse_args()
    
    store = MailStore(args.root, args.actor)
    
    if args.command == "inbox":
        print(json.dumps(store.inbox(), indent=2))
    elif args.command == "send":
        payload = json.loads(args.payload)
        res = store.send(args.to, args.type, payload, args.in_reply_to)
        print(json.dumps(res, indent=2))
    elif args.command == "recv":
        res = store.recv(args.type, args.timeout)
        print(json.dumps(res, indent=2))
    elif args.command == "try_recv":
        res = store.try_recv(args.type)
        print(json.dumps(res, indent=2))
    elif args.command == "post":
        payload = json.loads(args.payload)
        res = store.post(payload, args.type)
        print(json.dumps(res, indent=2))
    elif args.command == "claim":
        res = store.claim(args.ttl)
        print(json.dumps(res, indent=2))
    elif args.command == "complete":
        result = json.loads(args.result)
        res = store.complete(args.task_id, result)
        print(json.dumps(res, indent=2))

if __name__ == "__main__":
    main()
