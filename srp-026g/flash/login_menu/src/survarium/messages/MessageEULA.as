package survarium.messages
{
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.external.ExternalInterface;
   import flash.filters.DropShadowFilter;
   import scaleform.clik.controls.Label;
   import scaleform.clik.controls.ScrollBar;
   import scaleform.clik.controls.TextArea;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.ComponentEvent;
   import survarium.controls.Message;
   
   public class MessageEULA extends MessageAccept
   {
      
      public var textField:TextArea = new EulaTextArea();
      
      public var labelField:Label = new BigLabel();
      
      public var background:Sprite = new BackgroundEULA();
      
      public var scrollBar:ScrollBar = new SrvScrollBarSkinned();
      
      public function MessageEULA()
      {
         btnAccept = new SvButton1Focused();
         btnCancel = new SrvButtonSkinned();
         super();
         this.textField.name = "CCA";
         this.textField.text = "";
         this.textField.scrollBar = this.scrollBar;
         addChild(this.background);
         addChild(this.scrollBar);
         addChild(this.textField);
         addChild(this.labelField);
         this.scrollBar.x = this.background.width - 20;
         this.scrollBar.height = this.background.height - 140;
         this.labelField.x = 18;
         this.labelField.y = 5;
         this.textField.editable = false;
         var filters:Array = [new DropShadowFilter(2,45,0,1,3,3)];
         this.textField.x = 22;
         this.textField.y = 66;
         this.scrollBar.y = 70;
         this.textField.setSize(this.background.width - 55,this.background.height - 135);
         btnAccept.enabled = false;
         btnAccept.y = this.background.height - btnAccept.height - 12;
         addChild(btnAccept);
         btnCancel.x = this.background.width / 2 + 3;
         btnCancel.y = btnAccept.y;
         addChild(btnCancel);
         btnAccept.x = btnCancel.x - btnCancel.width - 3;
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         if(_data && this.textField)
         {
            this.textField.addEventListener(ComponentEvent.STATE_CHANGE,this.checkAcceptBtnState);
            trace("EUAL_LABEL::",_data.label_window);
            this.textField.text = _data.text;
            this.labelField.text = _data.label_window;
            addEventListener(Event.REMOVED_FROM_STAGE,this.onRemoved,false,1);
            this.textField.addEventListener(Event.SCROLL,this.checkAcceptBtn);
            this.textField.position = 0;
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
      
      override protected function draw() : void
      {
         super.draw();
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
         removeEventListener(Event.REMOVED_FROM_STAGE,this.onRemoved);
         Message.onClick(_id,"no");
         ExternalInterface.call("cancel_changes",_data.selectedIndex);
      }
      
      override protected function onMessageAccept(event:ButtonEvent = null) : void
      {
         if(!btnAccept.enabled)
         {
            return;
         }
         if(_data)
         {
            if(_data.onYes is Function)
            {
               _data.onYes();
            }
            _data.onNo = null;
            _data.onYes = null;
         }
         removeEventListener(Event.REMOVED_FROM_STAGE,this.onRemoved);
         Message.onClick(_id,"yes",_data);
      }
      
      protected function checkAcceptBtnState(e:Event = null) : void
      {
         this.textField.removeEventListener(ComponentEvent.STATE_CHANGE,this.checkAcceptBtnState);
         this.checkAcceptBtn();
      }
      
      protected function checkAcceptBtn(e:Event = null) : void
      {
         if(this.textField.position == this.textField.textField.maxScrollV)
         {
            this.textField.removeEventListener(Event.SCROLL,this.checkAcceptBtn);
            btnAccept.enabled = true;
         }
      }
      
      protected function onRemoved(e:Event) : void
      {
         this.onMessageCancel();
      }
   }
}

