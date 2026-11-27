package survarium.upgrade
{
   import com.greensock.TweenLite;
   import com.greensock.easing.Back;
   import flash.display.DisplayObject;
   import flash.display.Shape;
   import flash.display.Sprite;
   import flash.geom.Rectangle;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import flash.text.TextFormat;

   // One slot's marker over the gun, 0.69d0's SlotPoint and WeaponModuleSlot (its workshop's
   // ViewWeaponWindow): a dot at the slot's place on the gun, a ring while nothing stands there and
   // filled when something does, and a square off the gun with what stands there (its icon, else its
   // name) under the slot's name. A click on the square bubbles to the view, which opens the slot's
   // list. Both grow in when the marker is new (0.69d0's 0.5 s Back ease); the view places them and
   // draws the line between.
   public class SlotMarker extends Sprite
   {

      public static const SIZE:Number = 56;

      protected static const DOT:Number = 5;

      protected static const ICON_ROOM:Number = SIZE - 8;

      protected static const TITLE_GAP:Number = 2;

      protected static const LIGHT:uint = 0xE7F1EE;

      protected static const GREY:uint = 0xB4BEC4;

      protected static const BACK:uint = 0x0B1014;

      protected static const OPEN:uint = 0xF2C14E;

      public var slot:int;

      // what its content shows (ModificationView.fill's key), null before any
      public var content:String = null;

      public var dot:Shape;

      public var box:Sprite;

      public var title:TextField;

      protected var _face:Shape;

      protected var _content:Sprite;

      protected var _caption:TextField;

      protected var _filled:Boolean = false;

      protected var _open:Boolean = false;

      protected var _showPhase:Number = 1;

      public function SlotMarker(slot:int, name:String)
      {
         super();
         this.slot = slot;
         mouseEnabled = false;
         this.dot = new Shape();
         addChild(this.dot);
         this.box = new Sprite();
         this.box.buttonMode = true;
         this.box.mouseChildren = false;
         addChild(this.box);
         this._face = new Shape();
         this.box.addChild(this._face);
         this._content = new Sprite();
         this.box.addChild(this._content);
         this._caption = label(11,GREY,true);
         this._caption.width = ICON_ROOM;
         this.box.addChild(this._caption);
         this.title = label(13,LIGHT,false);
         this.title.text = name;
         this.title.x = -this.title.width / 2;
         this.box.addChild(this.title);
         this.draw();
      }

      protected static function label(size:int, colour:uint, wrap:Boolean) : TextField
      {
         var field:TextField = new TextField();
         var format:TextFormat = new TextFormat("Calibri",size,colour,true);
         format.align = "center";
         field.embedFonts = true;
         field.selectable = false;
         field.mouseEnabled = false;
         field.multiline = wrap;
         field.wordWrap = wrap;
         field.autoSize = TextFieldAutoSize.CENTER;
         field.defaultTextFormat = format;
         return field;
      }

      // What stands in the slot: its icon (the inventory's, scaled into the square) or, with none, its
      // name; `filled` fills the dot.
      public function setContent(icon:DisplayObject, text:String, filled:Boolean) : void
      {
         var scale:Number = 1;
         while(this._content.numChildren > 0)
         {
            this._content.removeChildAt(0);
         }
         this._caption.text = Boolean(icon) ? "" : text;
         this._caption.x = -ICON_ROOM / 2;
         this._caption.y = -this._caption.height / 2;
         if(Boolean(icon))
         {
            icon.scaleX = icon.scaleY = 1;
            scale = Math.min(1,ICON_ROOM / Math.max(1,icon.width),ICON_ROOM / Math.max(1,icon.height));
            icon.scaleX = icon.scaleY = scale;
            var bounds:Rectangle = icon.getBounds(icon);
            icon.x = -(bounds.x + bounds.width / 2) * scale;
            icon.y = -(bounds.y + bounds.height / 2) * scale;
            this._content.addChild(icon);
         }
         this._filled = filled;
         this.draw();
      }

      // The slot's list is open: its square lit.
      public function set open(value:Boolean) : void
      {
         this._open = value;
         this.draw();
      }

      public function placeDot(x:Number, y:Number) : void
      {
         this.dot.x = Math.round(x);
         this.dot.y = Math.round(y);
      }

      // The square's middle at (x, y); the slot's name above it (a square over the gun) or below.
      public function placeBox(x:Number, y:Number, titleAbove:Boolean) : void
      {
         this.box.x = Math.round(x);
         this.box.y = Math.round(y);
         this.title.y = titleAbove ? -SIZE / 2 - TITLE_GAP - this.title.height : SIZE / 2 + TITLE_GAP;
      }

      // Grown in from nothing after `delay` seconds.
      public function show(delay:Number) : void
      {
         TweenLite.killTweensOf(this);
         this.showPhase = 0;
         TweenLite.to(this,0.5,{
            "showPhase":1,
            "ease":Back.easeOut,
            "delay":delay
         });
      }

      public function get showPhase() : Number
      {
         return this._showPhase;
      }

      public function set showPhase(value:Number) : void
      {
         this._showPhase = value;
         this.dot.scaleX = this.dot.scaleY = value;
         this.box.scaleX = this.box.scaleY = value;
      }

      public function destroy() : void
      {
         TweenLite.killTweensOf(this);
      }

      protected function draw() : void
      {
         this.dot.graphics.clear();
         this.dot.graphics.lineStyle(1.5,LIGHT,0.9);
         this.dot.graphics.beginFill(this._filled ? LIGHT : BACK,this._filled ? 0.95 : 0.45);
         this.dot.graphics.drawCircle(0,0,DOT);
         this.dot.graphics.endFill();
         this._face.graphics.clear();
         this._face.graphics.lineStyle(1,this._open ? OPEN : LIGHT,this._open ? 1 : 0.65);
         this._face.graphics.beginFill(BACK,0.72);
         this._face.graphics.drawRect(-SIZE / 2,-SIZE / 2,SIZE,SIZE);
         this._face.graphics.endFill();
      }
   }
}
