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

      // pointing right (the next page) or left
      public function set pointRight(param1:Boolean) : void
      {
         this._right = param1;
         this._shown = null;
         this.show(_state);
      }

      override protected function setState(param1:String) : void
      {
         super.setState(param1);
         if(Boolean(this._face))
         {
            this.show(param1);
         }
      }

      protected function show(param1:String) : void
      {
         var _loc2_:Array = _stateMap ? _stateMap[param1] as Array : null;
         var _loc3_:String = Boolean(_loc2_) && _loc2_.length > 0 ? String(_loc2_[0]) : "up";
         if(!(_loc3_ in faces()))
         {
            _loc3_ = "up";
         }
         if(_loc3_ == this._shown)
         {
            return;
         }
         this._shown = _loc3_;
         this.drawFace(faces()[_loc3_] as BitmapData);
         this.drawGlyph(_loc3_ == "over" ? 15200750 : (_loc3_ == "down" ? 9213853 : (_loc3_ == "disabled" ? 5595242 : 11845316)));
      }

      protected function drawFace(param1:BitmapData) : void
      {
         var _loc2_:Number = HEIGHT / param1.height;
         var _loc3_:Number = CAP * _loc2_;
         var _loc4_:Number = (WIDTH - 2 * _loc3_) / (param1.width - 2 * CAP);
         this._face.graphics.clear();
         // a near-invisible fill under all: the whole button takes the mouse
         this._face.graphics.beginFill(0,0.01);
         this._face.graphics.drawRect(0,0,WIDTH,HEIGHT);
         this._face.graphics.endFill();
         this._face.graphics.beginBitmapFill(param1,new Matrix(_loc2_,0,0,_loc2_,0,0),false,true);
         this._face.graphics.drawRect(0,0,_loc3_,HEIGHT);
         this._face.graphics.endFill();
         this._face.graphics.beginBitmapFill(param1,new Matrix(_loc4_,0,0,_loc2_,_loc3_ - CAP * _loc4_,0),false,true);
         this._face.graphics.drawRect(_loc3_,0,WIDTH - 2 * _loc3_,HEIGHT);
         this._face.graphics.endFill();
         this._face.graphics.beginBitmapFill(param1,new Matrix(_loc2_,0,0,_loc2_,WIDTH - _loc3_ - (param1.width - CAP) * _loc2_,0),false,true);
         this._face.graphics.drawRect(WIDTH - _loc3_,0,_loc3_,HEIGHT);
         this._face.graphics.endFill();
      }

      // a triangle 6 wide and 10 high in the middle of the face (above its bottom shadow), with a shadow
      // a pixel down and right; fills only (a line style would draw from the pen)
      protected function drawGlyph(param1:uint) : void
      {
         var _loc2_:Number = WIDTH / 2;
         var _loc3_:Number = HEIGHT * 17 / 36;
         this._glyph.graphics.clear();
         this.triangle(_loc2_ + 1,_loc3_ + 1,0,0.6);
         this.triangle(_loc2_,_loc3_,param1,1);
      }

      protected function triangle(param1:Number, param2:Number, param3:uint, param4:Number) : void
      {
         var _loc5_:Number = this._right ? 1 : -1;
         this._glyph.graphics.beginFill(param3,param4);
         this._glyph.graphics.moveTo(param1 + 3 * _loc5_,param2);
         this._glyph.graphics.lineTo(param1 - 3 * _loc5_,param2 - 5);
         this._glyph.graphics.lineTo(param1 - 3 * _loc5_,param2 + 5);
         this._glyph.graphics.lineTo(param1 + 3 * _loc5_,param2);
         this._glyph.graphics.endFill();
      }
   }
}
