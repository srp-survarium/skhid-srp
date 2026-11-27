package survarium.upgrade
{
   import survarium.data.LocalizationData;

   // A collimator scope item's reticle (docs/notes/reticle-choice.md, "The build's contract"): what
   // the item shows (pattern, size, colour, brightness) and what it can show (its lists). The exe
   // gives a scope item it has lists for an AS3 member srp_reticle,
   // {pattern, size, colour, brightness, patterns: [name], sizes: n, colours: [name]}, which parse
   // reads once (InventoryList) into this; an item without it, or with a member out of the contract's
   // bounds, has no choice (null). A choice is never changed: a step is a new one (stepped), which
   // the window sends as srp_set_reticle(item, pattern, size, colour, brightness) (the exe's 46 05;
   // the server's Reticle, which refuses a value outside the scope's lists).
   public class ReticleChoice
   {

      // the steppers, in the window's order (ReticlePanel's rows)
      public static const PATTERN:int = 0;

      public static const SIZE:int = 1;

      public static const COLOUR:int = 2;

      public static const BRIGHTNESS:int = 3;

      public static const STEPPERS:Array = [PATTERN,SIZE,COLOUR,BRIGHTNESS];

      // brightness is 0..10 on every scope (0.69d0's), and does not wrap
      public static const BRIGHTNESS_MAX:int = 10;

      // the lists' bounds (the contract's data/srp_reticle)
      protected static const MAX_PATTERNS:int = 16;

      protected static const MAX_SIZES:int = 8;

      protected static const MAX_COLOURS:int = 8;

      // a pattern's or colour's name: its text's key is the prefix and the name
      protected static const NAME:RegExp = /^[a-z0-9_]{1,23}$/;

      protected static const LABELS:Array = ["st_srp_reticle_pattern","st_srp_reticle_size","st_srp_reticle_colour","st_srp_reticle_brightness"];

      protected static const PATTERN_TEXT:String = "st_srp_reticle_pattern_";

      protected static const COLOUR_TEXT:String = "st_srp_reticle_colour_";

      protected var _values:Array;

      protected var _patterns:Array;

      protected var _sizes:int;

      protected var _colours:Array;

      public function ReticleChoice(values:Array, patterns:Array, sizes:int, colours:Array)
      {
         super();
         this._values = values;
         this._patterns = patterns;
         this._sizes = sizes;
         this._colours = colours;
      }

      // The exe's srp_reticle member, or null for none or one out of bounds.
      public static function parse(raw:Object) : ReticleChoice
      {
         var patterns:Array = null;
         var colours:Array = null;
         var sizes:int = 0;
         var values:Array = null;
         var choice:ReticleChoice = null;
         var stepper:int = 0;
         if(raw == null || !(raw.patterns is Array) || !(raw.colours is Array))
         {
            return null;
         }
         patterns = names(raw.patterns as Array,MAX_PATTERNS);
         colours = names(raw.colours as Array,MAX_COLOURS);
         sizes = int(raw.sizes);
         if(patterns == null || colours == null || sizes < 1 || sizes > MAX_SIZES || Number(raw.sizes) != sizes)
         {
            return null;
         }
         values = [raw.pattern,raw.size,raw.colour,raw.brightness];
         choice = new ReticleChoice([int(raw.pattern),int(raw.size),int(raw.colour),int(raw.brightness)],patterns,sizes,colours);
         for each(stepper in STEPPERS)
         {
            if(Number(values[stepper]) != choice.value(stepper) || choice.value(stepper) < 0 || choice.value(stepper) >= choice.count(stepper))
            {
               return null;
            }
         }
         return choice;
      }

      // A list of 1..max names, or null.
      protected static function names(raw:Array, max:int) : Array
      {
         var out:Array = [];
         var name:* = null;
         if(raw.length < 1 || raw.length > max)
         {
            return null;
         }
         for each(name in raw)
         {
            if(!(name is String) || !NAME.test(String(name)))
            {
               return null;
            }
            out.push(String(name));
         }
         return out;
      }

      public function value(stepper:int) : int
      {
         return int(this._values[stepper]);
      }

      // How many values a stepper goes through.
      public function count(stepper:int) : int
      {
         return stepper == PATTERN ? int(this._patterns.length) : (stepper == SIZE ? this._sizes : (stepper == COLOUR ? int(this._colours.length) : BRIGHTNESS_MAX + 1));
      }

      // The choice one step on (`direction` 1) or back (-1): the pattern, size and colour wrap around
      // their lists, the brightness stops at 0 and BRIGHTNESS_MAX. This choice itself when the step
      // changes nothing (a list of one, the brightness at its end).
      public function stepped(stepper:int, direction:int) : ReticleChoice
      {
         var n:int = this.count(stepper);
         var next:int = this.value(stepper) + (direction < 0 ? -1 : 1);
         var values:Array = null;
         if(stepper == BRIGHTNESS)
         {
            next = Math.max(0,Math.min(BRIGHTNESS_MAX,next));
         }
         else
         {
            next = (next % n + n) % n;
         }
         if(next == this.value(stepper))
         {
            return this;
         }
         values = this._values.concat();
         values[stepper] = next;
         return new ReticleChoice(values,this._patterns,this._sizes,this._colours);
      }

      public function canStep(stepper:int, direction:int) : Boolean
      {
         return this.stepped(stepper,direction) != this;
      }

      // The same choice and lists.
      public function equals(other:ReticleChoice) : Boolean
      {
         return other != null && this._values.join(",") == other._values.join(",") && this._sizes == other._sizes && this._patterns.join(",") == other._patterns.join(",") && this._colours.join(",") == other._colours.join(",");
      }

      public static function labelKey(stepper:int) : String
      {
         return LABELS[stepper];
      }

      // A stepper's value as the window shows it: the pattern's and the colour's texts, the size as
      // its place in the list ("2/4"), the brightness as its number.
      public function text(stepper:int) : String
      {
         if(stepper == PATTERN)
         {
            return LocalizationData.getName(PATTERN_TEXT + this._patterns[this.value(PATTERN)]);
         }
         if(stepper == COLOUR)
         {
            return LocalizationData.getName(COLOUR_TEXT + this._colours[this.value(COLOUR)]);
         }
         if(stepper == SIZE)
         {
            return this.value(SIZE) + 1 + "/" + this._sizes;
         }
         return String(this.value(BRIGHTNESS));
      }
   }
}
