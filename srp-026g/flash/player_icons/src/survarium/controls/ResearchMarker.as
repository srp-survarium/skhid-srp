package survarium.controls
{
   import flash.display.Bitmap;
   import flash.display.Graphics;
   import flash.display.Shape;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.utils.getTimer;

   // Research (mode 5): a research unit's marker where the unit is on the
   // screen, as retail's (0.27d's player icon of kind 2, RoundProgress /
   // CapturePointMarker; research_mode.md "Markers in the retail game"): one
   // size at any distance, SIZE px (SIZE_INSIDE and the corner brackets while
   // the game's own player is inside), translucent, raised LIFT px, no label.
   // The unit's letter in a circle of its owner's colour (the game's own team
   // blue, the other red, nobody grey, as the hud's units) and a ring as far
   // round as the capture of the team taking it; blinking while a capture or
   // neutralisation runs; retail's crossed swords, pulsing, while both teams
   // are inside. The layer (HUDIconLayer.research_markers) places it.
   public class ResearchMarker extends Sprite
   {

      public static const OWN:uint = 0x3D9AE8;

      public static const ENEMY:uint = 0xE8443D;

      public static const NOBODY:uint = 0x8A8A8A;

      public static const NO_TEAM:uint = 2;

      // ---- the look, in screen pixels ----

      // The marker's width: retail's 24 px, 34 px while the game's own player
      // is inside (0.27d CapturePointMarker: 24 / 46 and 34 / 46 of its art).
      public static const SIZE:Number = 24;

      public static const SIZE_INSIDE:Number = 34;

      // World markers must leave players and scenery readable through them.
      // The HUD's separate units keep their stronger fill and never scope-fade.
      public static const ALPHA:Number = 0.55;

      public static const FILL_ALPHA:Number = 0.12;

      // Above the unit's point on the screen (retail's placer 0x234DA9: 5 px).
      public static const LIFT:Number = 5;

      // Smaller with distance: from SIZE at NEAR metres to SIZE x SMALLEST at
      // FAR and beyond. SMALLEST 1: one size, as retail (0.8 would cap the
      // shrink at a fifth).
      public static const NEAR:Number = 15;

      public static const FAR:Number = 150;

      public static const SMALLEST:Number = 1;

      // One fade out and back in takes two seconds at any frame rate.
      // Counting frames made the retail triangle blink too fast at high FPS.
      public static const PULSE_MS:uint = 2000;

      // ---- the drawing (its own units: the circle RADIUS + RING) ----

      protected static const RADIUS:Number = 12;

      protected static const RING:Number = 3;

      // retail's swords: 54 x 53 over its 46 px circle, at (-27, -26) x 1.016
      protected static const ART:Number = (RADIUS + RING) / 23;

      protected var _art:Sprite;

      protected var _icon:Shape;

      protected var _active:Shape;

      protected var _swords:Sprite;

      protected var _drawn:String = "";

      protected var _blink:Boolean = false;

      protected var _pulseStarted:uint = 0;

      public function ResearchMarker()
      {
         var bitmap:Bitmap;
         super();
         mouseEnabled = false;
         mouseChildren = false;
         alpha = ALPHA;
         this._art = new Sprite();
         addChild(this._art);
         this._icon = new Shape();
         this._art.addChild(this._icon);
         this._active = new Shape();
         drawActive(this._active.graphics,RADIUS + RING + 3);
         this._active.visible = false;
         this._art.addChild(this._active);
         this._swords = new Sprite();
         bitmap = new Bitmap(new ResearchSwords());
         bitmap.smoothing = true;
         bitmap.x = -27 * 1.016;
         bitmap.y = -26 * 1.016;
         bitmap.scaleX = bitmap.scaleY = 1.016;
         this._swords.addChild(bitmap);
         this._swords.scaleX = this._swords.scaleY = ART;
         this._swords.visible = false;
         this._art.addChild(this._swords);
      }

      public static function colourOf(team:uint, ownTeam:uint) : uint
      {
         if(team == NO_TEAM)
         {
            return NOBODY;
         }
         return team == ownTeam ? OWN : ENEMY;
      }

      // Retail's `active` highlight: four white corners around the circle.
      public static function drawActive(g:Graphics, half:Number) : void
      {
         var arm:Number = half * 0.35;
         var sx:int;
         var sy:int;
         g.lineStyle(2,0xFFFFFF,1);
         for each(sx in [-1,1])
         {
            for each(sy in [-1,1])
            {
               g.moveTo(sx * half,sy * (half - arm));
               g.lineTo(sx * half,sy * half);
               g.lineTo(sx * (half - arm),sy * half);
            }
         }
         g.lineStyle();
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

      // progress 0..100 of the team `capturing`'s capture; left: ms of the
      // capture or neutralisation running, 0 none; metres from the camera;
      // contested: both teams inside.
      public function update(cpId:uint, owner:uint, capturing:uint, progress:Number, left:Number, inside:Boolean, ownTeam:uint, metres:Number, contested:Boolean) : void
      {
         var near:Number = Math.max(0,Math.min(1,(metres - NEAR) / (FAR - NEAR)));
         var drawn:String = [cpId,owner,capturing,progress,ownTeam].join(",");
         var g:Graphics;
         scaleX = scaleY = (1 - (1 - SMALLEST) * near) * (inside ? SIZE_INSIDE : SIZE) / (2 * (RADIUS + RING));
         if(drawn != this._drawn)
         {
            this._drawn = drawn;
            g = this._icon.graphics;
            g.clear();
            g.lineStyle(1.5,colourOf(owner,ownTeam),1);
            g.beginFill(colourOf(owner,ownTeam),FILL_ALPHA);
            g.drawCircle(0,0,RADIUS);
            g.endFill();
            g.lineStyle();
            if(capturing != NO_TEAM && progress > 0 && !(owner == capturing && progress >= 100))
            {
               drawRing(g,RADIUS,RING,progress / 100,colourOf(capturing,ownTeam));
            }
            drawLetter(g,cpId,RADIUS);
         }
         this._active.visible = inside;
         if(contested && !this._swords.visible)
         {
            this._pulseStarted = uint(getTimer());
            this._swords.alpha = 1;
         }
         this._swords.visible = contested;
         this._blink = left > 0 && !contested;
         if(this._blink || contested)
         {
            if(!hasEventListener(Event.ENTER_FRAME))
            {
               this._pulseStarted = uint(getTimer());
               addEventListener(Event.ENTER_FRAME,this.onFrame);
            }
         }
         else if(hasEventListener(Event.ENTER_FRAME))
         {
            removeEventListener(Event.ENTER_FRAME,this.onFrame);
         }
         if(!this._blink)
         {
            this._art.alpha = 1;
         }
      }

      // Retail's triangle, timed in milliseconds: 1 down to 0 and back.
      // Unsigned subtraction also handles getTimer's wraparound.
      protected function onFrame(e:Event) : void
      {
         var wave:Number;
         var elapsed:uint = uint(uint(getTimer()) - this._pulseStarted);
         wave = Math.abs(1 - (elapsed % PULSE_MS) / PULSE_MS * 2);
         if(this._blink)
         {
            this._art.alpha = wave;
         }
         // Start visible when contested so the opponent is signalled at once.
         this._swords.alpha = wave;
      }
   }
}
