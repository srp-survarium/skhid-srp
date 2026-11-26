package survarium.upgrade
{
   import flash.display.BitmapData;
   import flash.display.Shape;
   import flash.geom.Matrix;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import flash.text.TextFormat;
   import survarium.buttons.ButtonClick;

   // One side of a second sight's toggle (UpgradeWindow's side buttons, under its slot): the window's
   // Confirm button art (button_*_bitmap, as PageArrow has it) at the button's size, its ends kept and
   // its middle stretched, with the side's word on it. The side the sight is on is `chosen`: the
   // pressed face and the light word, whatever the mouse does; a side the gun cannot take is disabled.
   public class SideButton extends ButtonClick
   {

      // the art's ends, in its own pixels
      protected static const CAP:Number = 5;

      protected static const LABEL_SIZE:int = 13;

      // the smallest size a long word is shrunk to (UpgradeWindow.fitText's)
      protected static const LABEL_MIN_SIZE:int = 9;

      protected static const LABEL_PADDING:Number = 4;

      // the word's colour: grey, lighter on hover and chosen, dim disabled
      protected static const LABEL_UP:uint = 0xB4BEC4;

      protected static const LABEL_LIGHT:uint = 0xE7F1EE;

      protected static const LABEL_DISABLED:uint = 0x55606A;

      protected static const HIT_ALPHA:Number = 0.01;

      protected static var _faces:Object;

      public var side:int;

      public var caption:TextField;

      protected var _face:Shape;

      protected var _w:Number;

      protected var _h:Number;

      protected var _chosen:Boolean = false;

      protected var _shown:String;

      protected var _text:String = "";

      // a button of `h` high; its width is the window's (resize)
      public function SideButton(side:int, w:Number, h:Number)
      {
         super();
         this.side = side;
         this._w = w;
         this._h = h;
         preventAutosizing = true;
         this._face = new Shape();
         addChild(this._face);
         this.caption = new TextField();
         this.caption.name = "sideLabel";
         this.caption.embedFonts = true;
         this.caption.selectable = false;
         this.caption.mouseEnabled = false;
         this.caption.multiline = false;
         this.caption.wordWrap = false;
         this.caption.autoSize = TextFieldAutoSize.NONE;
         this.caption.width = w - 2 * LABEL_PADDING;
         this.caption.x = LABEL_PADDING;
         addChild(this.caption);
         _originalWidth = _width = w;
         _originalHeight = _height = h;
         this.show("up");
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

      // The button's width (the window's: half its second sight's square), the word fitted to it again.
      public function resize(w:Number) : void
      {
         if(w == this._w)
         {
            return;
         }
         this._w = w;
         _originalWidth = _width = w;
         this.caption.width = w - 2 * LABEL_PADDING;
         this._shown = null;
         this.show(_state);
         this.text = this._text;
      }

      // The word, its size cut until it fits the button on one line.
      public function set text(value:String) : void
      {
         var size:int = LABEL_SIZE;
         this._text = value;
         this.caption.text = value;
         this.styleCaption(size);
         while((this.caption.textWidth + 4 > this.caption.width || this.caption.numLines > 1) && size > LABEL_MIN_SIZE)
         {
            size--;
            this.styleCaption(size);
         }
         this.caption.height = this.caption.textHeight + 4;
         this.caption.y = Math.round((this._h - this.caption.height) / 2) - 1;
      }

      public function get chosen() : Boolean
      {
         return this._chosen;
      }

      public function set chosen(value:Boolean) : void
      {
         this._chosen = value;
         this._shown = null;
         this.show(_state);
      }

      override public function set enabled(value:Boolean) : void
      {
         super.enabled = value;
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

      // The face shown: disabled, pressed when chosen, else the state's.
      public function get face() : String
      {
         var frames:Array = _stateMap ? _stateMap[_state] as Array : null;
         var face:String = Boolean(frames) && frames.length > 0 ? String(frames[0]) : "up";
         if(!enabled)
         {
            return "disabled";
         }
         if(this._chosen)
         {
            return "down";
         }
         return face in faces() ? face : "up";
      }

      protected function show(state:String) : void
      {
         var face:String = this.face;
         if(face == this._shown || !this._face)
         {
            return;
         }
         this._shown = face;
         this.drawFace(faces()[face] as BitmapData);
         this.styleCaption(int(this.caption.getTextFormat().size) > 0 ? int(this.caption.getTextFormat().size) : LABEL_SIZE);
      }

      protected function styleCaption(size:int) : void
      {
         var colour:uint = this._shown == "disabled" ? LABEL_DISABLED : (this._shown == "down" || this._shown == "over" ? LABEL_LIGHT : LABEL_UP);
         var format:TextFormat = new TextFormat("Calibri",size,colour,true);
         format.align = "center";
         this.caption.defaultTextFormat = format;
         this.caption.setTextFormat(format);
      }

      protected function drawFace(art:BitmapData) : void
      {
         var scale:Number = this._h / art.height;
         var cap:Number = CAP * scale;
         var stretch:Number = (this._w - 2 * cap) / (art.width - 2 * CAP);
         this._face.graphics.clear();
         this._face.graphics.beginFill(0,HIT_ALPHA);
         this._face.graphics.drawRect(0,0,this._w,this._h);
         this._face.graphics.endFill();
         this._face.graphics.beginBitmapFill(art,new Matrix(scale,0,0,scale,0,0),false,true);
         this._face.graphics.drawRect(0,0,cap,this._h);
         this._face.graphics.endFill();
         this._face.graphics.beginBitmapFill(art,new Matrix(stretch,0,0,scale,cap - CAP * stretch,0),false,true);
         this._face.graphics.drawRect(cap,0,this._w - 2 * cap,this._h);
         this._face.graphics.endFill();
         this._face.graphics.beginBitmapFill(art,new Matrix(scale,0,0,scale,this._w - cap - (art.width - CAP) * scale,0),false,true);
         this._face.graphics.drawRect(this._w - cap,0,cap,this._h);
         this._face.graphics.endFill();
      }
   }
}
