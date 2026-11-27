package survarium.player
{
   import survarium.constants.PlayerModifier;

   // The selected kit's own 68 player modifier floats, by index (survarium.constants.PlayerModifier):
   // what the exe sends with root.set_kit_modifiers before each weight callback
   // (client-patches/weight_ui_026g.c). srp puts the sums of the account's skills there; the kit's
   // items are not in them (the movie adds those itself).
   public final class KitModifiers
   {
      public static const COUNT:int = 68;

      private var _values:Array;

      // The medical skills' ranks a medicine card needs (docs/notes/medical-skills.md): from the
      // packed float 43 Potent Painkiller (bits 0..2) and Fast Bandaging (3..5); from float 45 Numb
      // Legs (0..1), Take Cover (2..3, no longer in the tree), Unflinching (4..6), Concentration
      // (7..9), Second Wind (10..11) and Sharp Eye (12..13).
      private var _painkillerRank:int;

      private var _bandagingRank:int;

      private var _numbLegsRank:int;

      private var _takeCoverRank:int;

      private var _unflinchingRank:int;

      private var _concentrationRank:int;

      private var _sharpEyeRank:int;

      // Resistance's rank (skill 502), bits 0..2 of the packed float 40
      // (docs/notes/artefact-skills.md, Transport).
      private var _resistanceRank:int;

      // The technical tree's digits in modifier 47, as technical_skills::FIELDS: [skill, ranks].
      private static const TECHNICAL:Array = [[401,3],[402,3],[403,2],[404,2],[411,3],[412,1],[413,2],[414,2],
         [422,1],[423,1],[431,3],[432,1],[433,2],[441,3],[442,1],[443,1],[444,1]];

      // The technical skills' ranks by id; none when float 47 is no tree's.
      private var _technical:Object = {};

      public function KitModifiers(values:Array)
      {
         _values = values;
         parseTechnical(values[PlayerModifier.TECHNICAL_RANKS]);
         // As the exe's artefact Skills::parse: float 40 an exact integer below 2^14, Resistance
         // (bits 0..2) at most 5 and Keeper's draw (9..11) at most 6, otherwise no artefact skill.
         var artefact:Object = packed(values[PlayerModifier.ARTEFACT_RANKS],14);
         if(artefact != null && (int(artefact) & 7) <= 5 && (int(artefact) >> 9 & 7) <= 6) _resistanceRank = int(artefact) & 7;
         // As the exe's medical_skills: float 43 an exact integer below 2^24 and float 45 one below
         // 2^14, each rank field at most its skill's ranks, otherwise no medical skill at all.
         var medical:Object = packed(values[PlayerModifier.MEDICAL_RANKS],24);
         var comboField:Object = packed(values[PlayerModifier.COMBO_RANKS],14);
         if(medical == null || comboField == null) return;
         var bits:int = int(medical);
         var combo:int = int(comboField);
         // [first bit, width, most ranks]: Potent Painkiller, Fast Bandaging, Metabolism, Medical
         // Training, Adrenalin, Strong Bones, Brothers in Arms; then Numb Legs, Take Cover, Unflinching,
         // Concentration, Second Wind, Sharp Eye.
         for each(var field:Array in [[0,3,3],[3,3,2],[6,3,4],[9,3,3],[12,3,3],[17,3,5],[21,3,1]])
         {
            if((bits >> field[0] & (1 << field[1]) - 1) > field[2]) return;
         }
         for each(field in [[0,2,2],[2,2,2],[4,3,4],[7,3,3],[10,2,3],[12,2,3]])
         {
            if((combo >> field[0] & (1 << field[1]) - 1) > field[2]) return;
         }
         _painkillerRank = bits & 7;
         _bandagingRank = bits >> 3 & 7;
         _numbLegsRank = combo & 3;
         _takeCoverRank = combo >> 2 & 3;
         _unflinchingRank = combo >> 4 & 7;
         _concentrationRank = combo >> 7 & 7;
         _sharpEyeRank = combo >> 12 & 3;
      }

      // As technical_skills::Skills::parse: a whole number of -LOW..LOW, LOW the product of the first
      // sixteen digits' radices; a negative one has Bear Trap (its rank takes LOW away); the rest
      // is a mixed-radix number; Tank Pocket and Tank Depth never both.
      private function parseTechnical(value:Number) : void
      {
         var low:Number = 1;
         for(var i:int = 0; i < TECHNICAL.length - 1; i++) low *= TECHNICAL[i][1] + 1;
         if(!(value >= -low && value < low) || value != Math.floor(value)) return;
         var ranks:Object = {};
         ranks[444] = value < 0 ? 1 : 0;
         var rest:Number = value < 0 ? value + low : value;
         for(i = 0; i < TECHNICAL.length - 1; i++)
         {
            var radix:int = TECHNICAL[i][1] + 1;
            ranks[TECHNICAL[i][0]] = rest % radix;
            rest = Math.floor(rest / radix);
         }
         if(ranks[413] > 0 && ranks[414] > 0) return;
         _technical = ranks;
      }

      public function technicalRank(skill:int) : int
      {
         return int(_technical[skill]);
      }

      // A packed float: an exact integer of `width` bits, else null.
      private static function packed(value:Number, width:int) : Object
      {
         return value >= 0 && value < (1 << width) && value == Math.floor(value) ? int(value) : null;
      }

      // The floats as the exe gives them; null unless they are 68 finite numbers.
      public static function parse(values:Array) : KitModifiers
      {
         if(values == null || values.length != COUNT) return null;
         var parsed:Array = [];
         for each(var value:* in values)
         {
            if(!(value is Number) || !isFinite(value)) return null;
            parsed.push(Number(value));
         }
         return new KitModifiers(parsed);
      }

      public function at(index:int) : Number
      {
         return _values[index];
      }

      public function get painkillerRank() : int
      {
         return _painkillerRank;
      }

      public function get bandagingRank() : int
      {
         return _bandagingRank;
      }

      public function get numbLegsRank() : int
      {
         return _numbLegsRank;
      }

      public function get takeCoverRank() : int
      {
         return _takeCoverRank;
      }

      public function get unflinchingRank() : int
      {
         return _unflinchingRank;
      }

      public function get concentrationRank() : int
      {
         return _concentrationRank;
      }

      public function get sharpEyeRank() : int
      {
         return _sharpEyeRank;
      }

      public function get resistanceRank() : int
      {
         return _resistanceRank;
      }
   }
}
