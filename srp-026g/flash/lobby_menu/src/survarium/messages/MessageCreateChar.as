package survarium.messages
{
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.external.ExternalInterface;
   import flash.filters.DropShadowFilter;
   import flash.text.TextField;
   import flash.text.TextFormat;
   import scaleform.clik.controls.TextInput;
   import scaleform.clik.events.ButtonEvent;
   
   public class MessageCreateChar extends MessageApply
   {
      
      protected var textFieldMoney:TextField = new TextField();
      
      protected var textFieldAction:TextField = new TextField();
      
      protected var textInput:TextInput = new TextInput();
      
      protected var coinIcon:MovieClip = new CoinsIcon2();
      
      public function MessageCreateChar()
      {
         super();
         background.width = 307;
         background.height = 180;
         addChild(background);
         var filters:Array = [new DropShadowFilter(2,45,0,1,3,3)];
         var frLabel:TextFormat = new TextFormat("Calibri",16,16777215);
         this.textInput.textField.filters = filters;
         this.textFieldAction.x = 22;
         this.textFieldAction.y = 90;
         this.textFieldAction.width = 300;
         this.textFieldAction.height = 30;
         this.textFieldAction.selectable = false;
         this.textFieldAction.defaultTextFormat = frLabel;
         this.textFieldAction.mouseEnabled = false;
         this.textFieldAction.filters = filters;
         this.textFieldMoney.x = 245;
         this.textFieldMoney.y = this.textFieldAction.y;
         this.textFieldMoney.width = 100;
         this.textFieldMoney.defaultTextFormat = frLabel;
         this.textFieldMoney.selectable = false;
         this.textFieldMoney.mouseEnabled = false;
         this.textFieldMoney.filters = filters;
         textField.mouseEnabled = false;
         this.coinIcon.gotoAndStop(2);
         addChild(this.coinIcon);
         this.textInput.defaultText = "";
         this.textInput.maxChars = 31;
         this.textInput.textField.addEventListener(Event.CHANGE,this.onChange);
         this.textInput.x = 40;
         this.textInput.y = 50;
         this.textInput.setSize(230,this.textInput.height);
         this.textFieldMoney.x = 245;
         this.coinIcon.x = this.textFieldMoney.x - 35;
         this.coinIcon.y = this.textFieldMoney.y;
         btnAccept.x = 28;
         btnAccept.y = background.height - 40;
         addChild(btnAccept);
         btnCancel.x = 196;
         btnCancel.y = background.height - 40;
         addChild(btnCancel);
         btnAccept.enabled = false;
         addChild(textField);
         addChild(this.textFieldMoney);
         addChild(this.textFieldAction);
         addChild(this.textInput);
      }
      
      protected function onChange(e:Event) : void
      {
         btnAccept.enabled = this.textInput.textField.length > 0;
      }
      
      override protected function configUI() : void
      {
         var total_row_width:uint;
         super.configUI();
         if(_data)
         {
            this.textFieldMoney.text = _data.createCost.toString();
            this.coinIcon.gotoAndStop(_data.createCostType + 1);
            textField.text = _data.text;
            this.textFieldAction.text = _data.stext;
            this.textInput.defaultText = _data.dtext;
            total_row_width = this.textFieldAction.textWidth + this.coinIcon.width + this.textFieldMoney.textWidth + 15;
            this.textFieldAction.x = (background.width - total_row_width) / 2;
            this.coinIcon.x = this.textFieldAction.x + this.textFieldAction.textWidth + 13;
            this.textFieldMoney.x = this.coinIcon.x + this.coinIcon.width + 2;
         }
      }
      
      override protected function onMessageCancel(event:ButtonEvent = null) : void
      {
         if(_data)
         {
            if(_data.onNo is Function)
            {
               _data.onNo();
            }
            _data.onNo = null;
            _data.onYes = null;
         }
         removeEventListener(Event.REMOVED_FROM_STAGE,onRemoved);
         super.onMessageCancel();
         ExternalInterface.call("cancel_changes",_data.selectedIndex);
      }
      
      override protected function onMessageAccept(event:ButtonEvent = null) : void
      {
         if(_data)
         {
            if(_data.onYes is Function)
            {
               _data.onYes(this.textInput.textField);
            }
            _data.onNo = null;
            _data.onYes = null;
         }
         removeEventListener(Event.REMOVED_FROM_STAGE,onRemoved);
         super.onMessageAccept();
         ExternalInterface.call("accept_changes",_data.selectedIndex);
      }
   }
}

