package survarium.controls
{
   import flash.display.Graphics;
   import flash.display.Bitmap;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.filters.GlowFilter;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import flash.text.TextFormat;
   import flash.utils.getTimer;

   // Research (mode 5): the research units under the teams' points. One circle
   // a unit, its letter, filled in the colour of its owner (the game's own
   // team blue, the other red, nobody grey), a ring as far round as the
   // capture of the team taking it, under a unit the time left of its
   // capture or neutralisation while one runs; the unit the game's own player
   // is inside larger (INSIDE), without a capture caption. The exe
   // (research_arena) sends the state with
   // HUD.research_update.
   public class ResearchUnits extends Sprite
   {

      public static const OWN:uint = 0x3D9AE8;

      public static const ENEMY:uint = 0xE8443D;

      public static const NOBODY:uint = 0x8A8A8A;

      public static const NO_TEAM:uint = 2;

      protected static const RADIUS:Number = 14;

      protected static const RING:Number = 4;

      // Room for a neighbour at INSIDE: 1.42 x (RADIUS + RING) + RADIUS + RING < SPACING.
      protected static const SPACING:Number = 48;

      // The unit the game's own player is inside: larger, as retail's active
      // highlight (0.27d CapturePointMarker: 34 / 46 against 24 / 46).
      public static const INSIDE:Number = 34 / 24;

      // From the bottom of the bars to the units' centres (HUD.placeResearchUnits), room for one at INSIDE.
      public static const TOP:Number = Math.ceil((RADIUS + RING) * INSIDE) + 4;

      protected var _units:Array = [];

      protected var _swords:Array = [];

      protected var _pulseStarts:Array = [];

      public static const PULSE_MS:uint = 2000;

      // A unit's time left, under its circle.
      protected var _timers:Array = [];

      protected static const TIMER_SIZE:Number = 16;

      public function ResearchUnits()
      {
         super();
         mouseEnabled = false;
         mouseChildren = false;
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

      // A unit's timer: DefaultLabel3's embedded font, smaller. The label
      // supplies the font only; it is never added to the display list.
      protected function makeTimer() : TextField
      {
         var source:TextField = new DefaultLabel3().textField;
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
         var shape:Sprite;
         var g:Graphics;
         var timer:TextField;
         var swords:Bitmap;
         var contested:Boolean;
         var anyContested:Boolean = false;
         while(this._units.length < units.length)
         {
            shape = new Sprite();
            addChild(shape);
            this._units.push(shape);
            swords = new Bitmap(new ResearchSwords());
            swords.smoothing = true;
            swords.scaleX = swords.scaleY = 2 * (RADIUS + RING) / 54;
            swords.x = -swords.width / 2;
            swords.y = -swords.height / 2;
            swords.visible = false;
            shape.addChild(swords);
            this._swords.push(swords);
            this._pulseStarts.push(uint(0));
            timer = this.makeTimer();
            addChild(timer);
            this._timers.push(timer);
         }
         while(this._units.length > units.length)
         {
            removeChild(this._units.pop());
            removeChild(this._timers.pop());
            this._swords.pop();
            this._pulseStarts.pop();
         }
         while(i < units.length)
         {
            unit = units[i];
            shape = this._units[i];
            shape.x = (i - (units.length - 1) / 2) * SPACING;
            shape.scaleX = shape.scaleY = unit.local ? INSIDE : 1;
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
            swords = this._swords[i];
            contested = unit.inside_0 > 0 && unit.inside_1 > 0;
            if(contested && !swords.visible)
            {
               this._pulseStarts[i] = uint(getTimer());
               swords.alpha = 1;
            }
            swords.visible = contested;
            anyContested = anyContested || contested;
            timer = this._timers[i];
            timer.visible = unit.left > 0;
            if(timer.visible)
            {
               timer.text = timeText(unit.left);
               timer.x = Math.round(shape.x - timer.width / 2);
               timer.y = Math.round((RADIUS + RING) * shape.scaleY);
            }
            i++;
         }
         if(anyContested)
         {
            addEventListener(Event.ENTER_FRAME,this.onContestedFrame);
         }
         else
         {
            removeEventListener(Event.ENTER_FRAME,this.onContestedFrame);
         }
      }

      // The top HUD keeps its own contested pulse even when the world
      // markers' parent has faded to zero for a scope.
      protected function onContestedFrame(e:Event) : void
      {
         var now:uint = uint(getTimer());
         var elapsed:uint;
         var i:uint = 0;
         while(i < this._swords.length)
         {
            if(this._swords[i].visible)
            {
               elapsed = uint(now - uint(this._pulseStarts[i]));
               this._swords[i].alpha = Math.abs(1 - (elapsed % PULSE_MS) / PULSE_MS * 2);
            }
            i++;
         }
      }
   }
}
