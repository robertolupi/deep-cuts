import os
import json
import time
import uuid
import shutil
from pathlib import Path
from typing import Dict, Any, Optional

# Coordination Base Directory (relative to repository root or custom path)
COORDINATION_DIR = Path("scratch/coordination")

class CoordinationAdapter:
    def __init__(self, actor_name: str, base_dir: Path = COORDINATION_DIR):
        self.actor_name = actor_name
        self.base_dir = base_dir
        
        # Define paths
        self.tmp_dir = self.base_dir / "tmp"
        self.actor_new_dir = self.base_dir / self.actor_name / "new"
        self.actor_cur_dir = self.base_dir / self.actor_name / "cur"
        self.tasks_dir = self.base_dir / "tasks"
        
        # Ensure directory structures exist
        self.tmp_dir.mkdir(parents=True, exist_ok=True)
        self.actor_new_dir.mkdir(parents=True, exist_ok=True)
        self.actor_cur_dir.mkdir(parents=True, exist_ok=True)
        self.tasks_dir.mkdir(parents=True, exist_ok=True)
        (self.tasks_dir / "open").mkdir(parents=True, exist_ok=True)
        (self.tasks_dir / "claimed").mkdir(parents=True, exist_ok=True)
        (self.tasks_dir / "completed").mkdir(parents=True, exist_ok=True)

    # ==========================================
    # Mailbox Operations (send / recv / try_recv)
    # ==========================================

    def send(self, to: str, msg_type: str, payload: Dict[str, Any], in_reply_to: Optional[str] = None) -> str:
        """
        Sends an asynchronous message to another actor using the Write-then-Rename pattern.
        """
        msg_id = str(uuid.uuid4())
        envelope = {
            "id": msg_id,
            "from": self.actor_name,
            "to": to,
            "type": msg_type,
            "payload": payload,
            "in_reply_to": in_reply_to,
            "ts": time.time()
        }
        
        # 1. Write to tmp/
        tmp_file = self.tmp_dir / f"msg_{msg_id}.json"
        with open(tmp_file, "w") as f:
            json.dump(envelope, f, indent=2)
            
        # 2. Atomic rename to recipient's new/ mailbox
        recipient_dir = self.base_dir / to / "new"
        recipient_dir.mkdir(parents=True, exist_ok=True)
        dest_file = recipient_dir / f"msg_{msg_id}.json"
        
        shutil.move(str(tmp_file), str(dest_file))
        return msg_id

    def try_recv(self) -> Optional[Dict[str, Any]]:
        """
        Non-blocking check for any unread messages.
        If a message exists, it is moved to cur/ (ACKed) and returned.
        """
        messages = sorted(self.actor_new_dir.glob("msg_*.json"), key=os.path.getmtime)
        if not messages:
            return None
            
        target_msg = messages[0]
        dest_file = self.actor_cur_dir / target_msg.name
        
        # Atomic move to cur/ (ACK)
        try:
            shutil.move(str(target_msg), str(dest_file))
            with open(dest_file, "r") as f:
                return json.load(f)
        except FileNotFoundError:
            # Race condition: another thread/process picked it up
            return None

    def recv(self, timeout: Optional[float] = None) -> Dict[str, Any]:
        """
        Blocking message read. Uses watchfiles if available for event-driven kernel wakeups,
        otherwise falls back to a low-overhead polling loop.
        """
        # Try non-blocking check first
        msg = self.try_recv()
        if msg:
            return msg
            
        start_time = time.time()
        poll_interval = 0.1
        
        # Attempt to use watchfiles library for native fswatch/inotify event blocking
        try:
            from watchfiles import watch
            # Watch the actor's new/ directory
            for changes in watch(self.actor_new_dir, stop_after_timeout=timeout):
                # Check for additions
                msg = self.try_recv()
                if msg:
                    return msg
        except ImportError:
            # Fall back to high-efficiency polling loop
            while True:
                msg = self.try_recv()
                if msg:
                    return msg
                    
                if timeout and (time.time() - start_time >= timeout):
                    raise TimeoutError("Recv timed out waiting for message.")
                    
                time.sleep(poll_interval)
                poll_interval = min(poll_interval * 1.5, 1.0)
                
        raise TimeoutError("Recv timed out waiting for message.")

    # ==========================================
    # Task Queue Operations (post / claim / complete)
    # ==========================================

    def post(self, task_id: str, task_type: str, payload: Dict[str, Any]) -> None:
        """
        Enqueues a new work task.
        """
        task_data = {
            "id": task_id,
            "type": task_type,
            "payload": payload,
            "status": "open",
            "posted_by": self.actor_name,
            "posted_at": time.time()
        }
        
        # Write to tmp/ first
        tmp_file = self.tmp_dir / f"{task_id}.json"
        with open(tmp_file, "w") as f:
            json.dump(task_data, f, indent=2)
            
        # Atomic rename to open/ task store
        dest_file = self.tasks_dir / "open" / f"{task_id}.json"
        shutil.move(str(tmp_file), str(dest_file))

    def claim(self, lease_ttl: Optional[float] = 120.0) -> Optional[Dict[str, Any]]:
        """
        Attempts to atomically claim a task from the open/ queue.
        Uses atomic filesystem moves to ensure exactly one winner.
        """
        open_tasks = sorted((self.tasks_dir / "open").glob("*.json"), key=os.path.getmtime)
        
        for task_path in open_tasks:
            # Destination path under claimed/
            dest_path = self.tasks_dir / "claimed" / task_path.name
            
            try:
                # Atomic rename acts as the mutual exclusion lock
                # os.rename fails on POSIX if destination exists, ensuring safety
                shutil.move(str(task_path), str(dest_path))
                
                # We won the race! Update task metadata with lease information
                with open(dest_path, "r+") as f:
                    task = json.load(f)
                    task["status"] = "claimed"
                    task["claimed_by"] = self.actor_name
                    task["claimed_at"] = time.time()
                    if lease_ttl:
                        task["lease_expires_at"] = time.time() + lease_ttl
                        
                    f.seek(0)
                    json.dump(task, f, indent=2)
                    f.truncate()
                    
                return task
            except (FileNotFoundError, PermissionError):
                # Someone else claimed it first, try the next task
                continue
                
        return None

    def heartbeat(self, task_id: str, lease_ttl: float = 120.0) -> None:
        """
        Extends the lease TTL on a currently claimed task.
        """
        task_path = self.tasks_dir / "claimed" / f"{task_id}.json"
        if not task_path.exists():
            raise FileNotFoundError(f"Claimed task {task_id} not found.")
            
        with open(task_path, "r+") as f:
            task = json.load(f)
            if task.get("claimed_by") != self.actor_name:
                raise PermissionError("Cannot heartbeat a task claimed by another actor.")
                
            task["lease_expires_at"] = time.time() + lease_ttl
            f.seek(0)
            json.dump(task, f, indent=2)
            f.truncate()

    def abandon(self, task_id: str, reason: str) -> None:
        """
        Releases a claimed task back to the open/ queue.
        """
        claimed_path = self.tasks_dir / "claimed" / f"{task_id}.json"
        if not claimed_path.exists():
            return
            
        # Write updated open state to tmp first
        tmp_file = self.tmp_dir / f"{task_id}.json"
        with open(claimed_path, "r") as f:
            task = json.load(f)
            
        task["status"] = "open"
        task.pop("claimed_by", None)
        task.pop("claimed_at", None)
        task.pop("lease_expires_at", None)
        task["abandoned_reason"] = reason
        task["abandoned_at"] = time.time()
        
        with open(tmp_file, "w") as f:
            json.dump(task, f, indent=2)
            
        # Atomic move to open/ and remove claimed file
        dest_path = self.tasks_dir / "open" / f"{task_id}.json"
        shutil.move(str(tmp_file), str(dest_path))
        try:
            claimed_path.unlink()
        except FileNotFoundError:
            pass

    def complete(self, task_id: str, result: Dict[str, Any]) -> None:
        """
        Marks a task as completed.
        """
        claimed_path = self.tasks_dir / "claimed" / f"{task_id}.json"
        if not claimed_path.exists():
            raise FileNotFoundError(f"Claimed task {task_id} not found.")
            
        # Write completed state to tmp first
        tmp_file = self.tmp_dir / f"{task_id}.json"
        with open(claimed_path, "r") as f:
            task = json.load(f)
            
        task["status"] = "completed"
        task["completed_at"] = time.time()
        task["result"] = result
        
        with open(tmp_file, "w") as f:
            json.dump(task, f, indent=2)
            
        # Atomic move to completed/ and remove claimed file
        dest_path = self.tasks_dir / "completed" / f"{task_id}.json"
        shutil.move(str(tmp_file), str(dest_path))
        try:
            claimed_path.unlink()
        except FileNotFoundError:
            pass
