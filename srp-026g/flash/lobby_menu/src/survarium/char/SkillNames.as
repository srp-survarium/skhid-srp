package survarium.char
{
   // The skills' names as the skill tree shows them: the exe gives them with each
   // skill's properties (root.fill_skills_properties: {id, name, ...}, already in
   // the player's language), which CharWindow records here by id. The movie's own
   // dictionary has no skill names.
   public final class SkillNames
   {
      private static var _names:Object = {};
      public static function record(id:uint, name:String) : void
      {
         if(name != null && name != "") _names[id] = name;
      }
      // The skill's name, or null before the exe has given the skills.
      public static function get(id:uint) : String
      {
         return _names[id] is String ? _names[id] : null;
      }
   }
}
