"""Finished worktrees must not be mistaken for live lane workers."""
import importlib.util,json,os,sys,tempfile,time,unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch
REPO=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location("worker_lanes",REPO/"tools/lanes.py")
lanes=importlib.util.module_from_spec(spec);sys.modules[spec.name]=lanes;spec.loader.exec_module(lanes)
class WorkerTests(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory();self.addCleanup(self.tmp.cleanup)
        self.root=Path(self.tmp.name);self.b=self.root/"build-runs";self.b.mkdir()
        self.ctx=SimpleNamespace(main=self.root,branch="main")
        self.wt=dict(path=self.root/"job",branch="job",head="unmerged");self.wt["path"].mkdir()
        self.reg=self.b/"run4-coordinator-state.json";self.result=self.b/"job-result.json"
    def register(self,e):self.reg.write_text(json.dumps({"lanes":{"job":e}}))
    def active(self,pid=None,age=0):
        with patch.object(lanes,"list_worktrees",return_value=[self.wt]),patch.object(lanes,"worktree_activity",return_value=time.time()-age),patch.object(lanes,"is_ancestor",return_value=False),patch.object(lanes,"worker_pid_live",return_value=pid):return lanes.active_lanes(self.ctx)
    def test_completed_even_if_pid_reused(self):
        self.result.write_text(json.dumps({"result":"ready"}));self.register({"pid":123,"result":str(self.result)})
        self.assertEqual(self.active(pid=True),[])
    def test_live_beyond_proxy_age(self):
        self.register({"pid":123});self.assertEqual(len(self.active(pid=True,age=7200)),1)
    def test_unknown_keeps_proxy(self):
        self.assertEqual(len(self.active()),1);self.assertEqual(self.active(age=7200),[])
    def test_malformed_registry_keeps_proxy(self):
        self.reg.write_text("{bad");self.assertEqual(len(self.active()),1)
    def test_partial_result_and_inaccessible_pid_keep_proxy(self):
        self.result.write_text("{partial");self.register({"pid":123,"result":str(self.result)})
        self.assertEqual(len(self.active()),1)
    def test_new_guard_invalidates_completion(self):
        self.result.write_text(json.dumps({"result":"ready"}));self.register({"result":str(self.result)})
        g=self.b/"lane-guard/job.json";g.parent.mkdir();g.write_text("{}");t=time.time();os.utime(self.result,(t-20,t-20));os.utime(g,(t,t))
        self.assertEqual(len(self.active(pid=False,age=7200)),1)
    def test_outside_result_keeps_proxy(self):
        p=self.root/"outside.json";p.write_text(json.dumps({"result":"ready"}));self.register({"result":str(p)})
        self.assertEqual(len(self.active(pid=False)),1)
    def test_dead_without_result_keeps_proxy(self):
        self.register({"pid":123});self.assertEqual(len(self.active(pid=False)),1)
