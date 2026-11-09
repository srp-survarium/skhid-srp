"""Appending skill records preserves the native table and nested value types."""
import unittest
import content_edit as ce

class AppendEntry(unittest.TestCase):
    def config(self):
        buf=bytearray(24)
        buf[:24]=ce.value_record(buf,0,'',{'skills_tree':{0:{'id':1,'ui':{'position':{0:2,1:1}},'modifiers':{0:{'speed':.05}}}}})
        return buf
    def test_nested_skill_and_unchanged_existing_records(self):
        buf=self.config();old={p:(t,v) for p,t,v in ce.walk(buf)}
        value={'id':301,'ui':{'position':[2,1]},'modifiers':[{'armored_trap_modifier':1.0}]*5}
        self.assertEqual(ce.append_entry(buf,'skills_tree',1,'id',value),1)
        new={p:(t,v) for p,t,v in ce.walk(buf)}
        for path,before in old.items():
            if path!='/skills_tree':self.assertEqual(new[path],before,path)
        self.assertEqual(ce.rec(buf,ce.find(buf,'skills_tree[1]/modifiers'))[3:],(ce.T_INDEXED,5))
        self.assertEqual(ce.rec(buf,ce.find(buf,'skills_tree[1]/modifiers[4]/armored_trap_modifier'))[3],ce.T_FLOAT)
    def test_stale_count_and_duplicate_refused_without_writes(self):
        for count,value in [(0,{'id':2}),(1,{'id':1}),(1,{'name':'no identity'})]:
            buf=self.config();before=bytes(buf)
            with self.assertRaises(ValueError):ce.append_entry(buf,'skills_tree',count,'id',value)
            self.assertEqual(bytes(buf),before)
    def test_vectors_and_empty_lists_as_the_games_objects_have_them(self):
        buf=self.config()
        value={'id':302,'position':[0.0,-1000.0,0.0],'containers':[],'pool':[{'dict_id':54,'count':20}]}
        ce.append_entry(buf,'skills_tree',1,'id',value,vectors=True)
        self.assertEqual(ce.rec(buf,ce.find(buf,'skills_tree[1]/position'))[3],ce.T_INDEXED+3)
        self.assertEqual(ce.rec(buf,ce.find(buf,'skills_tree[1]/containers'))[3:],(ce.T_INDEXED,0))
        self.assertEqual(ce.rec(buf,ce.find(buf,'skills_tree[1]/pool'))[3:],(ce.T_INDEXED,1))
        # Without vectors, as before: a list is an indexed table of numbers.
        buf=self.config()
        ce.append_entry(buf,'skills_tree',1,'id',value)
        self.assertEqual(ce.rec(buf,ce.find(buf,'skills_tree[1]/position'))[3:],(ce.T_INDEXED,3))

if __name__=='__main__':unittest.main()
