package survarium.messages
{
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.filters.DropShadowFilter;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import flash.text.TextFormat;
   import flash.text.TextFormatAlign;
   
   public class MessageNoClose extends MessageBase
   {
      
      protected var _text:String = "Hello I am test";
      
      protected const textOffset:uint = 32;
      
      public var _message:TextField = new TextField();
      
      public var _body:MovieClip = new MessageSmallBackground();
      
      public function MessageNoClose()
      {
         super();
         addChild(this._body);
         this._message.x = this.textOffset;
         this._message.y = this.textOffset;
         this._message.width = this._body.width - this.textOffset * 2;
         addChild(this._message);
         this._body.addEventListener(MouseEvent.MOUSE_DOWN,this._startDrag);
         addEventListener(MouseEvent.MOUSE_UP,this._endDrag);
         var frLabel:TextFormat = new TextFormat("Calibri",18,12568775,true);
         frLabel.align = TextFormatAlign.CENTER;
         var filters:Array = [new DropShadowFilter(2,90,0,1,3,3)];
         this._message.filters = filters;
         this._message.defaultTextFormat = frLabel;
         this._message.wordWrap = true;
         this._message.multiline = true;
         this._message.autoSize = TextFieldAutoSize.LEFT;
         this._message.selectable = false;
         this._message.mouseEnabled = false;
         this.text = this._text;
      }
      
      override protected function cleanUp(e:Event = null) : void
      {
         super.cleanUp();
         this._body.removeEventListener(MouseEvent.MOUSE_DOWN,this._startDrag);
         removeEventListener(MouseEvent.MOUSE_UP,this._endDrag);
      }
      
      public function set text(value:String) : void
      {
         _text = value;
         this._message.text = this._text;
         this.resize();
      }
      
      public function get text() : String
      {
         return this._text;
      }
      
      private function _startDrag(e:MouseEvent) : void
      {
         if(parent)
         {
            parent.addChild(this);
         }
         startDrag();
      }
      
      private function _endDrag(e:MouseEvent) : void
      {
         stopDrag();
      }
      
      protected function resize() : void
      {
         if(this._message.height > 400)
         {
            this._message.autoSize = TextFieldAutoSize.NONE;
            this._message.height = 400;
         }
         else
         {
            this._message.autoSize = TextFieldAutoSize.LEFT;
         }
         this._body.height = Math.max(this._message.height + 40,80);
      }
   }
}

