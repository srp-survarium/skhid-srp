package survarium.upgrade
{
   import flash.display.BitmapData;
   import flash.display.Shape;
   import flash.geom.Matrix;
   import survarium.buttons.ButtonClick;

   // The page arrow of a weapon's upgrade window: the window's Confirm button (its up, hover, pressed and
   // disabled art, button_*_bitmap) at arrow size, with an arrow on it. The art keeps its ends (5 px
   // of 119, at the height's scale) and stretches between them; the arrow is drawn, not stretched.
   public class PageArrow extends ButtonClick
   {

      public static const WIDTH:Number = 21;

      public static const HEIGHT:Number = 24;

      // the art's ends, in its own pixels
      protected static const CAP:Number = 5;

      // the arrow: a triangle 6 wide and 10 high, its middle at 17/36 of the height (above the face's
      // bottom shadow), with a shadow a pixel down and right
      protected static const ARROW_HALF_WIDTH:Number = 3;

      protected static const ARROW_HALF_HEIGHT:Number = 5;

      protected static const ARROW_MIDDLE:Number = 17 / 36;

      protected static const SHADOW_OFFSET:Number = 1;

      protected static const SHADOW_ALPHA:Number = 0.6;

      // the arrow's colour on each face: light grey, lighter on hover, darker pressed, dim disabled
      protected static const ARROW_UP:uint = 0xB4BEC4;

      protected static const ARROW_OVER:uint = 0xE7F1EE;

      protected static const ARROW_DOWN:uint = 0x8C979D;

      protected static const ARROW_DISABLED:uint = 0x55606A;

      // a near-invisible fill under the whole face: all of the button takes the mouse
      protected static const HIT_ALPHA:Number = 0.01;

      protected static var _faces:Object;

      protected var _face:Shape;

      protected var _glyph:Shape;

      protected var _right:Boolean = false;

      protected var _shown:String;

      // Built after CLIK's constructor (its preInitialize runs before the display object's own), then
      // given its size: CLIK measured an empty clip.
      public function PageArrow()
      {
         super();
         preventAutosizing = true;
         this._face = new Shape();
         this._glyph = new Shape();
         addChild(this._face);
         addChild(this._glyph);
         this.show("up");
         _originalWidth = _width = WIDTH;
         _originalHeight = _height = HEIGHT;
      }

      protected static function faces() : Object
      {
         if(!_faces)
         {
            _faces = {
               "up":new button_normal_bitmap(),
               "over":new button_hover_bitmap(),
               "down":new button_click_bitmap(),
               "disabled":new button_disabled_bitmap()
            };
         }
         return _faces;
      }

      protected static function arrowColour(face:String) : uint
      {
         return face == "over" ? ARROW_OVER : (face == "down" ? ARROW_DOWN : (face == "disabled" ? ARROW_DISABLED : ARROW_UP));
      }

      // pointing right (the next page) or left
      public function set pointRight(value:Boolean) : void
      {
         this._right = value;
         this._shown = null;
         this.show(_state);
      }

      override protected function setState(state:String) : void
      {
         super.setState(state);
         if(Boolean(this._face))
         {
            this.show(state);
         }
      }

      protected function show(state:String) : void
      {
         var frames:Array = _stateMap ? _stateMap[state] as Array : null;
         var face:String = Boolean(frames) && frames.length > 0 ? String(frames[0]) : "up";
         if(!(face in faces()))
         {
            face = "up";
         }
         if(face == this._shown)
         {
            return;
         }
         this._shown = face;
         this.drawFace(faces()[face] as BitmapData);
         this.drawGlyph(arrowColour(face));
      }

      protected function drawFace(art:BitmapData) : void
      {
         var scale:Number = HEIGHT / art.height;
         var cap:Number = CAP * scale;
         var stretch:Number = (WIDTH - 2 * cap) / (art.width - 2 * CAP);
         this._face.graphics.clear();
         this._face.graphics.beginFill(0,HIT_ALPHA);
         this._face.graphics.drawRect(0,0,WIDTH,HEIGHT);
         this._face.graphics.endFill();
         this._face.graphics.beginBitmapFill(art,new Matrix(scale,0,0,scale,0,0),false,true);
         this._face.graphics.drawRect(0,0,cap,HEIGHT);
         this._face.graphics.endFill();
         this._face.graphics.beginBitmapFill(art,new Matrix(stretch,0,0,scale,cap - CAP * stretch,0),false,true);
         this._face.graphics.drawRect(cap,0,WIDTH - 2 * cap,HEIGHT);
         this._face.graphics.endFill();
         this._face.graphics.beginBitmapFill(art,new Matrix(scale,0,0,scale,WIDTH - cap - (art.width - CAP) * scale,0),false,true);
         this._face.graphics.drawRect(WIDTH - cap,0,cap,HEIGHT);
         this._face.graphics.endFill();
      }

      // fills only (a line style would draw from the pen)
      protected function drawGlyph(colour:uint) : void
      {
         var middleX:Number = WIDTH / 2;
         var middleY:Number = HEIGHT * ARROW_MIDDLE;
         this._glyph.graphics.clear();
         this.triangle(middleX + SHADOW_OFFSET,middleY + SHADOW_OFFSET,0,SHADOW_ALPHA);
         this.triangle(middleX,middleY,colour,1);
      }

      protected function triangle(middleX:Number, middleY:Number, colour:uint, alpha:Number) : void
      {
         var direction:Number = this._right ? 1 : -1;
         this._glyph.graphics.beginFill(colour,alpha);
         this._glyph.graphics.moveTo(middleX + ARROW_HALF_WIDTH * direction,middleY);
         this._glyph.graphics.lineTo(middleX - ARROW_HALF_WIDTH * direction,middleY - ARROW_HALF_HEIGHT);
         this._glyph.graphics.lineTo(middleX - ARROW_HALF_WIDTH * direction,middleY + ARROW_HALF_HEIGHT);
         this._glyph.graphics.lineTo(middleX + ARROW_HALF_WIDTH * direction,middleY);
         this._glyph.graphics.endFill();
      }
   }
}
