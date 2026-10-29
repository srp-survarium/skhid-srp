package survarium.player
{
   import survarium.Util;
   import survarium.rooms.RoomsText;
   import survarium.data.LocalizationData;

   // Parsed once at the native callback. Fractions are the weight's contribution
   // to the game's modifier sums, with the selected kit's skills already applied.
   public final class WeightEffects
   {
      public static const WARNING_STYLE:String = "weight_warning";
      public static const WARNING_COLOR:String = "#F18A78";
      public static const SPRINT_ALLOWED:int = 0;
      public static const SPRINT_WEIGHT:int = 1;
      public static const SPRINT_HEAVYWEIGHT:int = 2;
      private var _movement:Number;
      private var _spending:Number;
      private var _sprint:int;
      private static const WORDS:Object = {
         "title":["Weight effects", "Вплив ваги", "Влияние веса", "Gewichtseffekte", "Wpływ wagi", "Uticaj težine"],
         "speed":["Movement speed", "Швидкість руху", "Скорость движения", "Bewegungsgeschwindigkeit", "Prędkość ruchu", "Brzina kretanja"],
         "spending":["Sprint stamina use", "Витрати витривалості під час спринту", "Расход выносливости при спринте", "Ausdauerverbrauch beim Sprinten", "Zużycie wytrzymałości podczas sprintu", "Potrošnja izdržljivosti pri sprintu"],
         "weight":["Cannot sprint: carry limit reached.", "Спринт недоступний: досягнуто межі ваги.", "Спринт недоступен: достигнут предел веса.", "Sprint nicht möglich: Traglast erreicht.", "Sprint niedostępny: osiągnięto limit wagi.", "Sprint nije moguć: dostignuta je nosivost."],
         "heavy":["Cannot sprint: Heavyweight skill.", "Спринт недоступний через навичку важковаговика.", "Спринт недоступен из-за навыка тяжеловеса.", "Sprint durch Schwergewicht-Fertigkeit gesperrt.", "Sprint zablokowany przez umiejętność ciężkiej wagi.", "Sprint nije moguć zbog veštine Teškaš."]
      };
      public function WeightEffects(movement:Number, spending:Number, sprint:int)
      {
         _movement = movement;
         _spending = spending;
         _sprint = sprint;
      }
      public static function parse(movement:Number, spending:Number, sprint:Number) : WeightEffects
      {
         if(!isFinite(movement) || !isFinite(spending) ||
            (sprint != SPRINT_ALLOWED && sprint != SPRINT_WEIGHT && sprint != SPRINT_HEAVYWEIGHT)) return null;
         return new WeightEffects(movement,spending,int(sprint));
      }
      public function get slowsMovement() : Boolean { return _movement < 0; }
      public function get title() : String { return word("title"); }
      private function word(key:String) : String { return WORDS[key][RoomsText.language()]; }
      private function percent(value:Number, bad:Boolean) : String
      {
         // One decimal, without float noise or a displayed negative zero.
         var magnitude:Number = Math.round(Math.abs(value) * 1000) / 10;
         // The game's embedded font has the ASCII minus, not U+2212.
         var text:String = (magnitude == 0 ? "" : value < 0 ? "-" : "+") + magnitude.toString() + "%";
         return Util.styledChar(text,bad ? "red" : "white");
      }
      private function stat(key:String, fallback:String) : String
      {
         var name:String = LocalizationData.getName(key);
         return name && name != key && name != "#" + key ? name : word(fallback);
      }
      // Same modifier atlas entries as the game's movement/energy defects
      // (modifications.options: defects 4070 and 4110).
      public function rows() : Array
      {
         var result:Array = [{icon:11,label:stat("st_inv_move_speed_modifier","speed") + ": " + percent(_movement,_movement < 0)}];
         if(_sprint != SPRINT_HEAVYWEIGHT)
            result.push({icon:16,label:stat("st_multiplicative_stamina_spending_modifier","spending") + ": " + percent(_spending,_spending > 0)});
         if(_sprint != SPRINT_ALLOWED)
            result.push({icon:11,label:Util.styledChar(word(_sprint == SPRINT_WEIGHT ? "weight" : "heavy"),"red")});
         return result;
      }
   }
}
