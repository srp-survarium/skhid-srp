package survarium.modes
{
   import flash.display.DisplayObject;
   import flash.display.Graphics;
   import flash.display.Shape;
   import flash.display.Sprite;
   import flash.geom.Rectangle;
   import flash.text.TextField;
   import survarium.controls.ResearchUnits;

   // Artifact Hunt's respawn wave beside the match time (docs/notes/artefact_hunt.md "HUD calls"
   // #21, #22): every team's next wave, for the living and the dead, while one comes. A dead
   // player's own time stays the game's respawn line (his modifier, the 7 s least time).
   // LOOK picks one of the candidate looks (the owner chooses; the others go before the merge).
   public class ArtefactHuntWave extends Sprite
   {
      // Dirty Bomb's: a smaller timer right after the main one; the bars make room.
      public static const AFTER:int = 1;
      // The same as "0:12".
      public static const AFTER_CLOCK:int = 2;
      // Under the main timer.
      public static const UNDER:int = 3;
      // Before the main timer.
      public static const BEFORE:int = 4;
      // A thin bar under the main timer, draining over the wave; no number.
      public static const BAR:int = 5;
      // The bar with the number under it.
      public static const BAR_NUMBER:int = 6;
      // A small ring after the main timer, draining, the seconds in it.
      public static const RING:int = 7;
      // After the main timer, only in the wave's last 5 s and for the dead.
      public static const LATE:int = 8;
      // After the main timer with a respawn arrow before the number.
      public static const GLYPH:int = 9;
      // Under each team's bar end, in its colour.
      public static const SIDES:int = 10;

      public static const LOOKS:int = 10;

      public static var LOOK:int = AFTER;

      // The wave's last ms drawn in the artefact's light (not Dirty Bomb's own look).
      protected static const WARM:Number = 3000;

      // LATE's ms.
      protected static const LATE_MS:Number = 5000;

      protected static const SIZE:Number = 20;

      protected static const RING_SIZE:Number = 15;

      // The digits' height in the font's top-to-baseline (the main timer's measured in Ruffle).
      protected static const DIGITS:Number = 0.64;

      protected static const GAP:Number = 3;

      protected static const RING_RADIUS:Number = 10;

      protected static const RING_WIDTH:Number = 2.5;

      protected var _number:TextField;

      protected var _twin:TextField;

      protected var _shape:Shape = new Shape();

      protected var _measure:TextField;

      // A dead viewer waits for his respawn (the game's respawn line shows).
      public var respawning:Boolean = false;

      public function ArtefactHuntWave(font:TextField)
      {
         super();
         this._number = ArtefactHuntLook.text(font,SIZE);
         this._twin = ArtefactHuntLook.text(font,SIZE);
         this._measure = ArtefactHuntLook.text(font,SIZE);
         addChild(this._shape);
         addChild(this._number);
         addChild(this._twin);
      }

      // Room the look takes beside the main timer, in this sprite's units: {before, after}.
      public function get room() : Object
      {
         var wide:Number = this.widest;
         switch(LOOK)
         {
            case AFTER:
            case AFTER_CLOCK:
            case LATE:
               return {before:0,after:wide + GAP};
            case GLYPH:
               return {before:0,after:wide + GAP + glyphWidth + 2};
            case RING:
               return {before:0,after:2 * (RING_RADIUS + RING_WIDTH) + GAP};
            case BEFORE:
               return {before:wide + GAP,after:0};
         }
         return {before:0,after:0};
      }

      protected function get widest() : Number
      {
         this._measure.text = LOOK == AFTER_CLOCK ? "0:00" : "00";
         return Math.ceil(this._measure.width);
      }

      protected function get glyphWidth() : Number
      {
         return SIZE * 0.6;
      }

      // How far under the main timer the look reaches (the row's artefact goes below it).
      public function get under() : Number
      {
         return visible ? this._under : 0;
      }

      protected var _under:Number = 0;

      protected var _clockBase:Number = 0;

      // At `now` beside `clock`: the main timer's text in the row's units, its bottom the digits'
      // baseline.
      public function draw(s:ArtefactHuntState, now:int, clock:Rectangle) : void
      {
         var left:Number = s == null ? 0 : s.waveLeftAt(now);
         var g:Graphics = this._shape.graphics;
         g.clear();
         this._under = 0;
         this._twin.visible = false;
         visible = left > 0 && clock != null && (LOOK != LATE || this.respawning || left <= LATE_MS);
         if(!visible)
         {
            return;
         }
         var seconds:String = String(Math.ceil(left / 1000));
         var text:String = LOOK == AFTER_CLOCK ? ResearchUnits.timeText(left) : seconds;
         var colour:uint = LOOK != AFTER && left <= WARM ? ArtefactHuntLook.GLOW : 0xFFFFFF;
         var part:Number = left / s.waveWhole;
         var n:TextField = this._number;
         n.visible = LOOK != BAR;
         this.setText(n,text,colour);
         var base:Number = clock.bottom;
         this._clockBase = base;
         // the main timer's digits: from `top` to its baseline
         var top:Number = base - DIGITS * clock.height;
         switch(LOOK)
         {
            case AFTER:
            case AFTER_CLOCK:
            case LATE:
               n.x = Math.round(clock.right + GAP);
               n.y = Math.round(base - baseline(n));
               break;
            case BEFORE:
               n.x = Math.round(clock.left - GAP - n.width);
               n.y = Math.round(base - baseline(n));
               break;
            case GLYPH:
               var arrow:Number = this.glyphWidth / 2;
               drawArrow(g,clock.right + GAP + arrow,base - DIGITS * baseline(n) / 2,arrow,colour);
               n.x = Math.round(clock.right + GAP + this.glyphWidth + 1);
               n.y = Math.round(base - baseline(n));
               break;
            case UNDER:
               n.x = Math.round(clock.x + clock.width / 2 - n.width / 2);
               this.below(n,base + 5);
               break;
            case BAR:
            case BAR_NUMBER:
               var bar:Rectangle = new Rectangle(Math.round(clock.x + 4),Math.round(base + 3),Math.round(clock.width - 8),2);
               g.beginFill(0,0.45);
               g.drawRect(bar.x - 1,bar.y - 1,bar.width + 2,bar.height + 2);
               g.endFill();
               g.beginFill(colour,0.95);
               g.drawRect(bar.x,bar.y,Math.max(1,Math.round(bar.width * part)),bar.height);
               g.endFill();
               this._under = bar.bottom + 1 - base;
               if(LOOK == BAR_NUMBER)
               {
                  n.x = Math.round(bar.x + bar.width / 2 - n.width / 2);
                  this.below(n,bar.bottom + 3);
               }
               break;
            case RING:
               var r:Number = RING_RADIUS + RING_WIDTH;
               var cx:Number = clock.right + GAP + r;
               var cy:Number = (top + base) / 2;
               g.beginFill(0,0.45);
               g.drawCircle(cx,cy,r + 1);
               g.endFill();
               ringPart(g,cx,cy,part,colour);
               this.setText(n,seconds,colour,RING_SIZE);
               n.x = Math.round(cx - n.width / 2);
               n.y = Math.round(cy + DIGITS * baseline(n) / 2 - baseline(n));
               break;
            case SIDES:
               var own:TextField = this._twin;
               own.visible = true;
               this.setText(own,seconds,left <= WARM ? ArtefactHuntLook.GLOW : ArtefactHuntLook.OWN);
               this.setText(n,seconds,left <= WARM ? ArtefactHuntLook.GLOW : ArtefactHuntLook.ENEMY);
               own.x = Math.round(clock.left - 4 - own.width);
               n.x = Math.round(clock.right + 4);
               this.below(n,(top + base) / 2 + 8);
               own.y = n.y;
               break;
         }
      }

      // `field`'s digits from `y` down; the look reaches as far under the main timer.
      protected function below(field:TextField, y:Number) : void
      {
         field.y = Math.round(y - (1 - DIGITS) * baseline(field));
         this._under = Math.max(this._under,y + DIGITS * baseline(field) + 2 - (this._clockBase));
      }

      // From a text's top to its baseline.
      protected static function baseline(field:TextField) : Number
      {
         return 2 + field.getLineMetrics(0).ascent;
      }

      // The box of what shows, in `space`'s units (getBounds counts hidden children too), for checks.
      public function shownBounds(space:DisplayObject) : Rectangle
      {
         var box:Rectangle = this._shape.getBounds(space);
         for each(var field:TextField in [this._number,this._twin])
         {
            if(field.visible)
            {
               box = box.isEmpty() ? field.getBounds(space) : box.union(field.getBounds(space));
            }
         }
         return box;
      }

      // The number shown ("" none, the bar's seconds for the bar alone), for checks.
      public function get text() : String
      {
         return !visible ? "" : (this._number.visible ? this._number.text : "bar");
      }

      protected function setText(field:TextField, text:String, colour:uint, size:Number = SIZE) : void
      {
         var format:* = field.defaultTextFormat;
         if(format.size != size || format.color != colour)
         {
            format.size = size;
            format.color = colour;
            field.defaultTextFormat = format;
            field.setTextFormat(format);
         }
         if(field.text != text)
         {
            field.text = text;
         }
      }

      // A ring round (cx, cy) as far round as `part`, from the top.
      protected static function ringPart(g:Graphics, cx:Number, cy:Number, part:Number, colour:uint) : void
      {
         var steps:int = Math.max(1,Math.ceil(part * 48));
         var r0:Number = RING_RADIUS;
         var r1:Number = RING_RADIUS + RING_WIDTH;
         var a:Number;
         var i:int;
         g.beginFill(colour,0.95);
         for(i = 0; i <= steps; i++)
         {
            a = -Math.PI / 2 + 2 * Math.PI * part * i / steps;
            if(i == 0)
            {
               g.moveTo(cx + Math.cos(a) * r1,cy + Math.sin(a) * r1);
            }
            else
            {
               g.lineTo(cx + Math.cos(a) * r1,cy + Math.sin(a) * r1);
            }
         }
         for(i = steps; i >= 0; i--)
         {
            a = -Math.PI / 2 + 2 * Math.PI * part * i / steps;
            g.lineTo(cx + Math.cos(a) * r0,cy + Math.sin(a) * r0);
         }
         g.endFill();
      }

      // The respawn arrow: three quarters of a circle and its head.
      protected static function drawArrow(g:Graphics, cx:Number, cy:Number, r:Number, colour:uint) : void
      {
         var steps:int = 24;
         var a:Number;
         g.lineStyle(Math.max(1.5,r * 0.32),colour,1);
         for(var i:int = 0; i <= steps; i++)
         {
            a = -Math.PI * 0.35 + 1.6 * Math.PI * i / steps;
            if(i == 0)
            {
               g.moveTo(cx + Math.cos(a) * r,cy + Math.sin(a) * r);
            }
            else
            {
               g.lineTo(cx + Math.cos(a) * r,cy + Math.sin(a) * r);
            }
         }
         g.lineStyle();
         a = -Math.PI * 0.35;
         var tip:Number = r * 0.75;
         var px:Number = cx + Math.cos(a) * r;
         var py:Number = cy + Math.sin(a) * r;
         g.beginFill(colour,1);
         g.moveTo(px + tip,py - tip * 0.15);
         g.lineTo(px - tip * 0.55,py - tip * 0.75);
         g.lineTo(px - tip * 0.25,py + tip * 0.6);
         g.endFill();
      }
   }
}
