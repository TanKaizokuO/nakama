import argparse
import json
import os
import platform
import sys
from dataclasses import asdict

from session import SessionLoader
from inventory import InventoryLoader
from routing import RoutingEngine
from query import QueryEngine
from transcript import TranscriptStore
from audit import ParityAuditor

def main():
    parser = argparse.ArgumentParser(description="Nakama Python Companion Workspace")
    subparsers = parser.add_subparsers(dest="command")

    subparsers.add_parser("rendersummary")
    subparsers.add_parser("showmanifest")
    subparsers.add_parser("setupreport")
    subparsers.add_parser("commandgraph")
    subparsers.add_parser("toolpool")
    subparsers.add_parser("bootstrapgraph")
    subparsers.add_parser("listsubsystems")
    subparsers.add_parser("parityaudit")
    subparsers.add_parser("listcommands")
    subparsers.add_parser("listtools")

    cmd = subparsers.add_parser("routeprompt")
    cmd.add_argument("prompt", type=str)
    
    cmd = subparsers.add_parser("bootstrapsession")
    cmd.add_argument("prompt", type=str)
    
    cmd = subparsers.add_parser("turnloop")
    cmd.add_argument("prompt", type=str)
    
    cmd = subparsers.add_parser("flushtranscript")
    cmd.add_argument("prompt", type=str)
    
    cmd = subparsers.add_parser("loadsession")
    cmd.add_argument("session_id", type=str)

    for mode in ["remotemode", "sshmode", "teleportmode", "directconnectmode", "deeplinkmode"]:
        cmd = subparsers.add_parser(mode)
        cmd.add_argument("target", type=str)

    cmd = subparsers.add_parser("showcommand")
    cmd.add_argument("name", type=str)
    
    cmd = subparsers.add_parser("showtool")
    cmd.add_argument("name", type=str)
    
    cmd = subparsers.add_parser("executecommand")
    cmd.add_argument("name", type=str)
    cmd.add_argument("prompt", type=str)
    
    cmd = subparsers.add_parser("executetool")
    cmd.add_argument("name", type=str)
    cmd.add_argument("payload", type=str)

    args = parser.parse_args()

    workspace_path = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    inv = InventoryLoader(workspace_path)
    routing = RoutingEngine(inv)

    if args.command == "setupreport":
        result = {
            "platform": {
                "os": platform.system(),
                "version": platform.version(),
                "arch": platform.machine(),
                "python_version": platform.python_version()
            },
            "prefetch_results": [],
            "deferred_init": []
        }
        print(json.dumps(result))
    elif args.command == "parityaudit":
        auditor = ParityAuditor(workspace_path, inv)
        result = auditor.audit()
        print(json.dumps(asdict(result)))
    elif args.command == "loadsession":
        loader = SessionLoader(workspace_path)
        try:
            meta, _, _ = loader.load_session(args.session_id)
            print(json.dumps(asdict(meta)))
        except Exception as e:
            print(json.dumps({"error": str(e)}), file=sys.stderr)
            sys.exit(1)
    elif args.command in ["remotemode", "sshmode", "teleportmode", "directconnectmode", "deeplinkmode"]:
        print(json.dumps({"mode": args.command, "target": args.target, "connected": False, "detail": "not implemented"}))
    elif args.command == "routeprompt":
        matches = routing.route(args.prompt)
        print(json.dumps([asdict(m) for m in matches]))
    elif args.command == "flushtranscript":
        ts = TranscriptStore(workspace_path)
        ts.append(args.prompt)
        path = ts.flush()
        print(json.dumps({"path": path, "flushed": True}))
    else:
        print(json.dumps({"status": "ok", "command": args.command}))

if __name__ == "__main__":
    main()
