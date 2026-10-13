import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location('bridge', Path(__file__).parents[1]/'tools/activity_bridge.py')
bridge = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bridge)

class ProjectionTests(unittest.TestCase):
    def test_no_titles_goals_ids_or_private_history_leave_provider(self):
        value=bridge.summary({'schema':'telecodex.sessions-dashboard/v1','observed_at':10,
            'sessions':[{'status':'active','title':'private','id':'secret','goal':{'objective':'private'}},
                        {'status':'unknown'},{'status':'idle'}]})
        self.assertEqual(value['active'],1)
        self.assertEqual(value['unconfirmed'],1)
        self.assertNotIn('private',str(value))
        self.assertNotIn('secret',str(value))
    def test_incompatible_source_fails(self):
        with self.assertRaises(ValueError): bridge.summary({'schema':'other'})

if __name__=='__main__': unittest.main()
