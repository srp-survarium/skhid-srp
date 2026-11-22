package survarium.replay
{
   import survarium.constants.TeamSide;

   // The exe's one-string forms of the root's lists (set_markers_text, set_players_text): GFx's
   // Invoke passes a string more easily than an Array of objects. Entries are joined by ";", their
   // fields by ":" (the last field, a text, is the rest of the entry: it may hold ":", never ";");
   // a malformed entry is left out, an empty string is an empty list.
   public class ReplayText
   {
      // `time_ms:kind:whose:actor:victim;...` (kind 1 a kill, 2 an objective, 3 a death of the player
      // in view; whose, the Moments switch's: 1 the viewer's own, 2 the player in view's, 3 both, 0
      // neither; actor and victim places, either empty): the markers [{time, kind, whose, label}] in
      // time order, each label `actor > victim` (or the one there is) of `names`, as set_markers_list
      // takes them (the prev/next label buttons then step from one to the next).
      public static function markers(value:String, names:Array = null) : Array
      {
         var list:Array = [];
         for each(var entry:String in ReplayText.entries(value))
         {
            var f:Array = ReplayText.fields(entry,5);
            if(f != null && ReplayText.isNumber(f[0]) && ReplayText.isNumber(f[1]) && ReplayText.isNumber(f[2])
               && ReplayText.isPlace(f[3]) && ReplayText.isPlace(f[4]))
            {
               list.push({
                  "time":uint(f[0]),
                  "kind":uint(f[1]),
                  "whose":uint(f[2]),
                  "label":ReplayText.label(f[3],f[4],names)
               });
            }
         }
         list.sortOn("time",Array.NUMERIC);
         return list;
      }

      // `place:name;...`: the names a marker's places show, as an Array indexed by place.
      public static function names(value:String) : Array
      {
         var list:Array = [];
         for each(var entry:String in ReplayText.entries(value))
         {
            var f:Array = ReplayText.fields(entry,2);
            if(f != null && ReplayText.isNumber(f[0]) && uint(f[0]) < 256)
            {
               list[uint(f[0])] = String(f[1]);
            }
         }
         return list;
      }

      // A marker's label: `actor > victim`, or the one there is, or nothing.
      private static function label(actor:String, victim:String, names:Array) : String
      {
         var a:String = ReplayText.nameOf(actor,names);
         var v:String = ReplayText.nameOf(victim,names);
         return a != "" && v != "" ? a + " > " + v : (a != "" ? a : v);
      }

      private static function nameOf(place:String, names:Array) : String
      {
         return place != "" && names != null && names[uint(place)] != undefined ? String(names[uint(place)]) : "";
      }

      private static function isPlace(text:String) : Boolean
      {
         return text == "" || ReplayText.isNumber(text);
      }

      // `place:team:name;...` (place 0..19, what select_player sends back; team the match's 0 or 1,
      // shown as ally and enemy): the players [{id, team, name}], as set_players_list takes them.
      public static function players(value:String) : Array
      {
         var list:Array = [];
         for each(var entry:String in ReplayText.entries(value))
         {
            var f:Array = ReplayText.fields(entry,3);
            if(f != null && ReplayText.isNumber(f[0]) && uint(f[0]) <= 19 && (f[1] == "0" || f[1] == "1") && f[2].length > 0)
            {
               list.push({
                  "id":uint(f[0]),
                  "team":f[1] == "0" ? TeamSide.ALLY : TeamSide.ENEMY,
                  "name":String(f[2])
               });
            }
         }
         return list;
      }

      private static function entries(value:String) : Array
      {
         return value == null || value.length == 0 ? [] : value.split(";");
      }

      // The `count` fields of an entry, the last the rest of it; null if it has fewer.
      private static function fields(entry:String, count:int) : Array
      {
         var out:Array = [];
         var from:int = 0;
         while(out.length < count - 1)
         {
            var at:int = entry.indexOf(":",from);
            if(at < 0)
            {
               return null;
            }
            out.push(entry.substring(from,at));
            from = at + 1;
         }
         out.push(entry.substring(from));
         return out;
      }

      private static function isNumber(text:String) : Boolean
      {
         return /^[0-9]{1,10}$/.test(text) && Number(text) <= uint.MAX_VALUE;
      }
   }
}
