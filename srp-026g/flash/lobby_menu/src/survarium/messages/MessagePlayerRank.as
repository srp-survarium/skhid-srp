package survarium.messages
{
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.text.TextField;
   import scaleform.clik.events.ButtonEvent;
   import survarium.controls.Message;
   import survarium.data.LocalizationData;
   
   public class MessagePlayerRank extends MessageAccept
   {
      
      public var textFieldLabel:TextField;
      
      public var textFieldLabelGold:TextField;
      
      public var textFieldLabelPoints:TextField;
      
      public var icon:MovieClip;
      
      public function MessagePlayerRank()
      {
         super();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         btnAccept.label = String(LocalizationData.getName("open") + " «" + LocalizationData.getName("character") + "»").toUpperCase();
         btnCancel.label = LocalizationData.getName("close");
         if(_data)
         {
            // A level pays silver or nothing: the gold the game's levels paid is gone (the owner,
            // 2026-11-08: "there is also gold given for levels, I would remove that too").
            this.icon.gotoAndStop(1);
            this.textFieldLabel.text = LocalizationData.getName("new_level_reached");
            this.textFieldLabelGold.text = _data.reward_amount + " " + LocalizationData.getName("regular_money");
            this.textFieldLabelGold.textColor = 12498861;
            this.icon.visible = this.textFieldLabelGold.visible = _data.reward_amount > 0;
            this.textFieldLabelPoints.text = "+1" + " " + LocalizationData.getName("point");
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
         Message.onClick(_id,"no");
      }
      
      override protected function onMessageAccept(event:ButtonEvent = null) : void
      {
         if(_data)
         {
            if(_data.onYes is Function)
            {
               _data.onYes();
            }
            _data.onNo = null;
            _data.onYes = null;
         }
         dispatchEvent(new Event(Event.OPEN));
         Message.onClick(_id,"yes",_data);
      }
   }
}

