package survarium.messages
{
   import flash.display.BitmapData;
   import flash.display.Graphics;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.filters.DropShadowFilter;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import flash.text.TextFormat;
   import flash.text.TextFormatAlign;
   import scaleform.clik.events.ButtonEvent;
   
   public class MessageApply extends MessageAccept
   {
      
      protected var _patternSprite:Sprite = new Sprite();
      
      protected var _patternOverSprite:Sprite = new Sprite();
      
      protected var _pattern:BitmapData = new ContextPattern();
      
      protected const textOffset:uint = 32;
      
      protected const offset:uint = 10;
      
      protected const buttonsSpace:uint = 1;
      
      public var textField:TextField = new TextField();
      
      public var background:Sprite = new MessageSmallBackground();
      
      public function MessageApply()
      {
         super();
         addChild(this.background);
         this.background.width = 300;
         var frLabel:TextFormat = new TextFormat("Calibri",18,12568775,true);
         frLabel.align = TextFormatAlign.CENTER;
         var filters:Array = [new DropShadowFilter(2,90,0,1,3,3)];
         this.textField.x = this.textOffset;
         this.textField.y = this.textOffset;
         this.textField.width = this.background.width - this.textOffset * 2;
         this.textField.selectable = false;
         frLabel.align = TextFormatAlign.CENTER;
         this.textField.wordWrap = true;
         this.textField.multiline = true;
         this.textField.autoSize = TextFieldAutoSize.LEFT;
         this.textField.defaultTextFormat = frLabel;
         this.textField.filters = filters;
         addChild(this.textField);
         addChild(btnAccept);
         addChild(btnCancel);
      }
      
      override protected function cleanUp(e:Event = null) : void
      {
         super.cleanUp(e);
         removeEventListener(Event.REMOVED_FROM_STAGE,this.onRemoved);
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         if(_data && this.textField)
         {
            this.textField.text = _data.text;
            addEventListener(Event.REMOVED_FROM_STAGE,this.onRemoved);
            if(_data)
            {
               if(_data.label_ok)
               {
                  btnAccept.label = _data.label_ok;
                  btnAccept.validateNow();
               }
               if(_data.label_cancel)
               {
                  btnCancel.label = _data.label_cancel;
                  btnCancel.validateNow();
               }
            }
            if(btnAccept.textField.textWidth > btnAccept.textField.width)
            {
               btnAccept.autoSize = "left";
               btnAccept.validateNow();
               btnCancel.width = btnAccept.width;
            }
            this.updateLayout();
            this.updatePattern();
         }
      }
      
      protected function updateLayout() : void
      {
         this.background.width = Math.max(this.textField.width + this.textOffset * 2,150);
         this.background.height = Math.max(this.textField.textHeight + this.textOffset * 2 + btnAccept.height + this.offset,80);
         btnAccept.x = this.background.width / 2 - btnAccept.width - this.buttonsSpace / 2;
         btnAccept.y = this.background.height - btnAccept.height - this.offset;
         btnCancel.x = this.background.width / 2 + this.buttonsSpace / 2;
         btnCancel.y = btnAccept.y;
      }
      
      protected function updatePattern() : void
      {
         var offset:uint = 8;
         var _body:Sprite;
         var gr:Graphics;
         var rectW:int;
         var rectH:int;
         var offSize:int;
         if(this._pattern && this._patternSprite)
         {
            _body = this.background;
            if(btnAccept.x < 10)
            {
               offSize = -(btnAccept.x - 10);
               _body.width += 2 * offSize;
               _body.x -= offSize;
            }
            this._patternSprite.alpha = 0.2;
            this._patternSprite.x = _body.x + offset - 1;
            this._patternSprite.y = _body.y + offset;
            this._patternOverSprite.x = this._patternSprite.x;
            this._patternOverSprite.y = this._patternSprite.y;
            gr = this._patternSprite.graphics;
            gr.clear();
            rectW = _body.width - offset * 2;
            rectH = _body.height - offset * 2;
            gr.beginBitmapFill(this._pattern,null,true,true);
            gr.drawRect(0,0,rectW,rectH);
            gr.endFill();
            gr = this._patternOverSprite.graphics;
            gr.clear();
            gr.beginFill(0,0.7);
            gr.drawRect(0,0,rectW,rectH);
            gr.endFill();
         }
      }
      
      override protected function onMessageCancel(event:ButtonEvent = null) : void
      {
         super.onMessageCancel();
         if(_data)
         {
            if(_data.onNo is Function)
            {
               _data.onNo();
            }
            _data.onNo = null;
            _data.onYes = null;
         }
      }
      
      override protected function onMessageAccept(event:ButtonEvent = null) : void
      {
         var fn:Function = null;
         super.onMessageAccept();
         if(_data)
         {
            fn = _data.onYes as Function;
            _data.onNo = null;
            _data.onYes = null;
            if(fn is Function)
            {
               fn(_data);
            }
         }
      }
      
      protected function onRemoved(e:Event) : void
      {
         this.onMessageCancel(null);
      }
   }
}

