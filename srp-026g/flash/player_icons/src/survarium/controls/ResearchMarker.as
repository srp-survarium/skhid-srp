package survarium.controls
{
   import flash.display.Graphics;
   import flash.display.Shape;
   import flash.display.Sprite;
   import flash.filters.GlowFilter;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import flash.text.TextFormat;

   // Research (mode 5): a research unit's marker where the unit is on the
   // screen, as 0.27d's player icon of kind 2 (RoundProgress): the unit's
   // letter in a circle of its owner's colour (the game's own team blue, the
   // other red, nobody grey, as the hud's units), a ring as far round as the
   // capture of the team taking it, larger while the game's own player is
   // inside. Smaller with distance; under it the time left of a capture or
   // neutralisation while one runs, else the distance in metres (not at the
   // interface level Minimal: textShown). The layer
   // (HUDIconLayer.research_markers) places it.
   public class ResearchMarker extends Sprite
   {

      public static const OWN:uint = 0x3D9AE8;

      public static const ENEMY:uint = 0xE8443D;

      public static const NOBODY:uint = 0x8A8A8A;

      public static const NO_TEAM:uint = 2;

      protected static const RADIUS:Number = 12;

      protected static const RING:Number = 3;

      // From full size at NEAR metres to SMALLEST at FAR and beyond.
      protected static const NEAR:Number = 15;

      protected static const FAR:Number = 150;

      protected static const SMALLEST:Number = 0.55;

      // 0.27d's marker of the unit the player is in: 34 / 46 against 24 / 46.
      protected static const INSIDE:Number = 1.4;

      protected var _icon:Shape;

      protected var _label:TextField;

      protected var _drawn:String = "";

      // The interface's level shows the text under the marker (HUDIconLayer.set_interface_level).
      public function set textShown(value:Boolean) : void
      {
         this._label.visible = value;
      }

      public function ResearchMarker(format:TextFormat)
      {
         super();
         mouseEnabled = false;
         mouseChildren = false;
         this._icon = new Shape();
         this._icon.alpha = 0.9;
         addChild(this._icon);
         this._label = new TextField();
         this._label.defaultTextFormat = format;
         this._label.embedFonts = true;
         this._label.selectable = false;
         this._label.mouseEnabled = false;
         this._label.multiline = false;
         this._label.wordWrap = false;
         this._label.autoSize = TextFieldAutoSize.CENTER;
         this._label.filters = [new GlowFilter(0,0.9,3,3,3)];
         addChild(this._label);
      }

      public static function colourOf(team:uint, ownTeam:uint) : uint
      {
         if(team == NO_TEAM)
         {
            return NOBODY;
         }
         return team == ownTeam ? OWN : ENEMY;
      }

      // "0:12": the seconds left, rounded up.
      public static function timeText(ms:Number) : String
      {
         var seconds:int = Math.ceil(ms / 1000);
         var minutes:int = int(seconds / 60);
         seconds = seconds % 60;
         return minutes + ":" + (seconds < 10 ? "0" : "") + seconds;
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
      // capture or neutralisation running, 0 none; metres from the camera.
      public function update(cpId:uint, owner:uint, capturing:uint, progress:Number, left:Number, inside:Boolean, ownTeam:uint, metres:Number) : void
      {
         var near:Number = Math.max(0,Math.min(1,(metres - NEAR) / (FAR - NEAR)));
         var drawn:String = [cpId,owner,capturing,progress,inside,ownTeam].join(",");
         var g:Graphics;
         var text:String = left > 0 ? timeText(left) : Math.round(metres) + " m";
         var scale:Number = (1 - (1 - SMALLEST) * near) * (inside ? INSIDE : 1);
         scaleX = scaleY = scale;
         if(drawn != this._drawn)
         {
            this._drawn = drawn;
            g = this._icon.graphics;
            g.clear();
            g.beginFill(inside ? 0xFFFFFF : 0,inside ? 0.8 : 0.45);
            g.drawCircle(0,0,RADIUS + RING);
            g.endFill();
            g.beginFill(colourOf(owner,ownTeam),0.9);
            g.drawCircle(0,0,RADIUS);
            g.endFill();
            if(capturing != NO_TEAM && progress > 0 && !(owner == capturing && progress >= 100))
            {
               drawRing(g,RADIUS,RING,progress / 100,colourOf(capturing,ownTeam));
            }
            drawLetter(g,cpId,RADIUS);
         }
         if(this._label.text != text)
         {
            this._label.text = text;
            this._label.textColor = left > 0 ? 0xFFFFFF : 0xC8C8C8;
         }
         // the text keeps its size at any distance
         this._label.scaleX = this._label.scaleY = 1 / scale;
         this._label.x = -this._label.width / 2;
         this._label.y = RADIUS + RING;
      }
   }
}
