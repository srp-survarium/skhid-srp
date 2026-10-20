package survarium.messages
{
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.external.ExternalInterface;
   import flash.filters.DropShadowFilter;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import flash.text.TextFormat;
   import flash.text.TextFormatAlign;
   import scaleform.clik.events.ButtonEvent;
   
   public class MessageExit extends MessageAccept
   {
      
      public var textField:TextField = new TextField();
      
      public var background:Sprite = new MessageSmallBackground();
      
      public function MessageExit()
      {
         super();
         trace("EXIT::part_0");
         addChild(this.background);
         var filters:Array = [new DropShadowFilter(2,45,0,1,3,3)];
         var frLabel:TextFormat = new TextFormat("Calibri",16,16777215);
         this.textField.x = 18;
         this.textField.y = 17;
         this.textField.width = 275;
         this.textField.selectable = false;
         frLabel.align = TextFormatAlign.CENTER;
         this.textField.wordWrap = true;
         this.textField.multiline = true;
         this.textField.autoSize = TextFieldAutoSize.LEFT;
         this.textField.defaultTextFormat = frLabel;
         this.textField.filters = filters;
         addChild(this.textField);
         btnAccept.x = 11;
         btnAccept.y = this.background.height - btnAccept.height - 11;
         addChild(btnAccept);
         btnCancel.x = this.background.width - btnCancel.width - 11;
         btnCancel.y = btnAccept.y;
         addChild(btnCancel);
      }
      
      override protected function configUI() : void
      {
         trace("EXIT::part_1");
         super.configUI();
         trace("EXIT::part_2");
         if(_data && this.textField)
         {
            this.textField.text = _data.text;
            this.textField.y = (this.background.height - btnAccept.y - this.textField.textHeight) / 2;
            addEventListener(Event.REMOVED_FROM_STAGE,this.onRemoved);
            if(_data)
            {
               if(_data.label_ok)
               {
                  btnAccept.label = _data.label_ok;
               }
               if(_data.label_cancel)
               {
                  btnCancel.label = _data.label_cancel;
               }
            }
         }
      }
      
      override protected function cleanUp(e:Event = null) : void
      {
         super.cleanUp(e);
         removeEventListener(Event.REMOVED_FROM_STAGE,this.onRemoved);
      }
      
      override protected function onMessageCancel(event:ButtonEvent = null) : void
      {
         removeEventListener(Event.REMOVED_FROM_STAGE,this.onRemoved);
         trace("EXIT::part_z2");
         if(_data)
         {
            if(_data.onNo is Function)
            {
               _data.onNo();
            }
            _data.onNo = null;
            _data.onYes = null;
            ExternalInterface.call("cancel_changes",_data.selectedIndex);
         }
         super.onMessageCancel();
      }
      
      override protected function onMessageAccept(event:ButtonEvent = null) : void
      {
         var fn:Function;
         removeEventListener(Event.REMOVED_FROM_STAGE,this.onRemoved);
         trace("EXIT::part_z1");
         super.onMessageAccept();
         ExternalInterface.call("accept_changes",_data.selectedIndex);
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
         this.onMessageCancel();
      }
   }
}

