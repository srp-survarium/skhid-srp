package survarium.controls
{
   import flash.display.Graphics;
   import flash.display.Shape;
   import flash.display.Sprite;
   import flash.filters.GlowFilter;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import flash.text.TextFormat;
   import survarium.data.LocalizationData;

   // Research (mode 5): the research units under the teams' points. One circle
   // a unit, its letter, filled in the colour of its owner (the game's own
   // team blue, the other red, nobody grey), a ring as far round as the
   // capture of the team taking it, under a unit the time left of its
   // capture or neutralisation while one runs, and under them "Capturing
   // unit..." while the game's own player is inside a unit his team is
   // taking. The exe (research_native_026g.c) sends the state with
   // HUD.research_update.
   public class ResearchUnits extends Sprite
   {

      public static const OWN:uint = 0x3D9AE8;

      public static const ENEMY:uint = 0xE8443D;

      public static const NOBODY:uint = 0x8A8A8A;

      public static const NO_TEAM:uint = 2;

      protected static const RADIUS:Number = 14;

      protected static const RING:Number = 4;

      protected static const SPACING:Number = 40;

      // From the bottom of the bars to the units' centres (HUD.placeResearchUnits).
      public static const TOP:Number = RADIUS + RING + 8;

      protected var _units:Array = [];

      protected var _caption:DefaultLabel3;

      // A unit's time left, under its circle.
      protected var _timers:Array = [];

      protected static const TIMER_SIZE:Number = 16;

      public function ResearchUnits()
      {
         super();
         mouseEnabled = false;
         mouseChildren = false;
         this._caption = new DefaultLabel3();
         // As wide as its text, on one line. The symbol's field (Open Sans
         // Condensed bold 30, 75 px wide) wraps: "Capturing unit..." broke
         // inside its first word, the field's one line showed "Captu", and
         // autoSize, which sizes to the widest line, kept it 75 px. The
         // longest text, German's "Erfassen der Installation...", has 28
         // letters; the font has every one of the five languages'.
         this._caption.textField.wordWrap = false;
         this._caption.textField.multiline = false;
         this._caption.autoSize = TextFieldAutoSize.CENTER;
         this._caption.y = RADIUS + RING + 6;
         this._caption.visible = false;
         addChild(this._caption);
      }

      public static function colourOf(team:uint, ownTeam:uint) : uint
      {
         if(team == NO_TEAM)
         {
            return NOBODY;
         }
         return team == ownTeam ? OWN : ENEMY;
      }

      // A unit's letter by its cp_id: A, B, C as 0.27d named them.
      public static function drawLetter(g:Graphics, cpId:uint, size:Number) : void
      {
         var h:Number = size;
         var w:Number = size * 0.7;
         g.lineStyle(2,0xFFFFFF,1);
         if(cpId == 0)
         {
            g.moveTo(-w / 2,h / 2);
            g.lineTo(0,-h / 2);
            g.lineTo(w / 2,h / 2);
            g.moveTo(-w / 4,h / 8);
            g.lineTo(w / 4,h / 8);
         }
         else if(cpId == 1)
         {
            g.moveTo(-w / 2,h / 2);
            g.lineTo(-w / 2,-h / 2);
            g.lineTo(w / 6,-h / 2);
            g.curveTo(w / 2,-h / 2,w / 2,-h / 4);
            g.curveTo(w / 2,0,w / 6,0);
            g.lineTo(-w / 2,0);
            g.moveTo(w / 6,0);
            g.curveTo(w / 2,0,w / 2,h / 4);
            g.curveTo(w / 2,h / 2,w / 6,h / 2);
            g.lineTo(-w / 2,h / 2);
         }
         else
         {
            g.moveTo(w / 2,-h / 3);
            g.curveTo(w / 3,-h / 2,0,-h / 2);
            g.curveTo(-w / 2,-h / 2,-w / 2,0);
            g.curveTo(-w / 2,h / 2,0,h / 2);
            g.curveTo(w / 3,h / 2,w / 2,h / 3);
         }
         g.lineStyle();
      }

      // The ring: an arc from the top, clockwise, `part` of the way round.
      public static function drawRing(g:Graphics, radius:Number, width:Number, part:Number, colour:uint) : void
      {
         var steps:int = Math.max(1,Math.ceil(part * 48));
         var i:int = 0;
         var a:Number;
         if(part <= 0)
         {
            return;
         }
         g.beginFill(colour,1);
         while(i <= steps)
         {
            a = -Math.PI / 2 + 2 * Math.PI * part * i / steps;
            if(i == 0)
            {
               g.moveTo(Math.cos(a) * (radius + width),Math.sin(a) * (radius + width));
            }
            else
            {
               g.lineTo(Math.cos(a) * (radius + width),Math.sin(a) * (radius + width));
            }
            i++;
         }
         i = steps;
         while(i >= 0)
         {
            a = -Math.PI / 2 + 2 * Math.PI * part * i / steps;
            g.lineTo(Math.cos(a) * radius,Math.sin(a) * radius);
            i--;
         }
         g.endFill();
      }

      // "0:12": the seconds left, rounded up.
      public static function timeText(ms:Number) : String
      {
         var seconds:int = Math.ceil(ms / 1000);
         var minutes:int = int(seconds / 60);
         seconds = seconds % 60;
         return minutes + ":" + (seconds < 10 ? "0" : "") + seconds;
      }

      // A unit's timer: the caption's font (DefaultLabel3's), smaller.
      protected function makeTimer() : TextField
      {
         var source:TextField = this._caption.textField;
         var format:TextFormat = source.length > 0 ? source.getTextFormat() : source.defaultTextFormat;
         var timer:TextField = new TextField();
         format.size = TIMER_SIZE;
         format.color = 0xFFFFFF;
         timer.defaultTextFormat = format;
         timer.embedFonts = true;
         timer.selectable = false;
         timer.mouseEnabled = false;
         timer.multiline = false;
         timer.wordWrap = false;
         timer.autoSize = TextFieldAutoSize.CENTER;
         timer.filters = [new GlowFilter(0,0.9,3,3,3)];
         timer.visible = false;
         return timer;
      }

      // units: [{cp_id, owner, capturing, progress (0..100), inside_0, inside_1, local, left (ms)}]
      public function update(units:Array, ownTeam:uint) : void
      {
         var i:int = 0;
         var unit:Object;
         var shape:Shape;
         var g:Graphics;
         var timer:TextField;
         var capturing:Boolean = false;
         var timed:Boolean = false;
         while(this._units.length < units.length)
         {
            shape = new Shape();
            addChild(shape);
            this._units.push(shape);
            timer = this.makeTimer();
            addChild(timer);
            this._timers.push(timer);
         }
         while(this._units.length > units.length)
         {
            removeChild(this._units.pop());
            removeChild(this._timers.pop());
         }
         while(i < units.length)
         {
            unit = units[i];
            shape = this._units[i];
            shape.x = (i - (units.length - 1) / 2) * SPACING;
            g = shape.graphics;
            g.clear();
            g.beginFill(0,0.45);
            g.drawCircle(0,0,RADIUS + RING);
            g.endFill();
            g.beginFill(colourOf(unit.owner,ownTeam),0.9);
            g.drawCircle(0,0,RADIUS);
            g.endFill();
            if(unit.capturing != NO_TEAM && unit.progress > 0 && !(unit.owner == unit.capturing && unit.progress >= 100))
            {
               drawRing(g,RADIUS,RING,unit.progress / 100,colourOf(unit.capturing,ownTeam));
            }
            drawLetter(g,unit.cp_id,RADIUS);
            timer = this._timers[i];
            timer.visible = unit.left > 0;
            if(timer.visible)
            {
               timed = true;
               timer.text = timeText(unit.left);
               timer.x = Math.round(shape.x - timer.width / 2);
               timer.y = RADIUS + RING;
            }
            if(unit.local && unit.capturing == ownTeam && !(unit.owner == ownTeam && unit.progress >= 100))
            {
               capturing = true;
            }
            i++;
         }
         if(capturing)
         {
            this._caption.text = LocalizationData.getName("research_capturing");
            this._caption.validateNow();
            this._caption.x = -Math.round(this._caption.width / 2);
            // under the timers when one shows
            this._caption.y = RADIUS + RING + 6 + (timed ? TIMER_SIZE + 2 : 0);
         }
         this._caption.visible = capturing;
      }
   }
}
