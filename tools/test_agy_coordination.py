import unittest
import shutil
import tempfile
import time
from pathlib import Path
from agy_coordination_adapter import CoordinationAdapter

class TestCoordinationAdapter(unittest.TestCase):
    def setUp(self):
        # Create a temporary directory for coordination mailspool
        self.test_dir = Path(tempfile.mkdtemp())
        self.adapter_a = CoordinationAdapter("actor_a", base_dir=self.test_dir)
        self.adapter_b = CoordinationAdapter("actor_b", base_dir=self.test_dir)

    def tearDown(self):
        # Clean up the temporary directory
        shutil.rmtree(self.test_dir)

    def test_send_recv_happy_path(self):
        # Actor A sends a message to Actor B
        msg_payload = {"command": "test", "data": 42}
        msg_id = self.adapter_a.send(to="actor_b", msg_type="TEST_CMD", payload=msg_payload)
        
        # Check that the file was atomically written and renamed into actor_b's new folder
        new_path = self.test_dir / "actor_b" / "new" / f"msg_{msg_id}.json"
        self.assertTrue(new_path.exists())
        
        # Actor B performs a non-blocking check
        msg = self.adapter_b.try_recv()
        self.assertIsNotNone(msg)
        self.assertEqual(msg["id"], msg_id)
        self.assertEqual(msg["from"], "actor_a")
        self.assertEqual(msg["to"], "actor_b")
        self.assertEqual(msg["payload"], msg_payload)
        
        # Check that it moved to cur/ (ACKed)
        cur_path = self.test_dir / "actor_b" / "cur" / f"msg_{msg_id}.json"
        self.assertTrue(cur_path.exists())
        self.assertFalse(new_path.exists())

    def test_blocking_recv_timeout(self):
        # Actor A blocks on recv with a very short timeout
        with self.assertRaises(TimeoutError):
            self.adapter_a.recv(timeout=0.1)

    def test_task_queue_operations(self):
        # Enqueue a task
        task_payload = {"file": "src/main.rs", "action": "test"}
        self.adapter_a.post(task_id="task_001", task_type="UNIT_TEST", payload=task_payload)
        
        # Task file should exist in tasks/open/
        open_path = self.test_dir / "tasks" / "open" / "task_001.json"
        self.assertTrue(open_path.exists())
        
        # Actor B claims the task
        task = self.adapter_b.claim(lease_ttl=10.0)
        self.assertIsNotNone(task)
        self.assertEqual(task["id"], "task_001")
        self.assertEqual(task["claimed_by"], "actor_b")
        self.assertEqual(task["status"], "claimed")
        
        # File should now exist in claimed/ and not in open/
        claimed_path = self.test_dir / "tasks" / "claimed" / "task_001.json"
        self.assertTrue(claimed_path.exists())
        self.assertFalse(open_path.exists())
        
        # Actor B sends a heartbeat
        initial_expiration = task["lease_expires_at"]
        time.sleep(0.1)
        self.adapter_b.heartbeat(task_id="task_001", lease_ttl=20.0)
        
        # Check that lease got updated
        with open(claimed_path, "r") as f:
            updated_task = json_load = json = __import__("json").load(f)
            self.assertGreater(updated_task["lease_expires_at"], initial_expiration)
            
        # Actor B completes the task
        self.adapter_b.complete(task_id="task_001", result={"exit_code": 0})
        
        # File should now exist in completed/ and not in claimed/
        completed_path = self.test_dir / "tasks" / "completed" / "task_001.json"
        self.assertTrue(completed_path.exists())
        self.assertFalse(claimed_path.exists())

    def test_task_abandonment(self):
        # Enqueue a task
        self.adapter_a.post(task_id="task_002", task_type="BUILD", payload={})
        
        # Actor B claims it
        self.adapter_b.claim(lease_ttl=10.0)
        
        # Actor B hits an error and abandons the task
        self.adapter_b.abandon(task_id="task_002", reason="Missing dependency")
        
        # Task should be back in open/ and have the abandon notes
        open_path = self.test_dir / "tasks" / "open" / "task_002.json"
        self.assertTrue(open_path.exists())
        
        with open(open_path, "r") as f:
            task = __import__("json").load(f)
            self.assertEqual(task["status"], "open")
            self.assertEqual(task["abandoned_reason"], "Missing dependency")
            self.assertNotIn("claimed_by", task)

if __name__ == "__main__":
    unittest.main()
