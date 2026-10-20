package survarium.messages
{
   import scaleform.clik.controls.Button;
   import scaleform.clik.events.ButtonEvent;
   import survarium.controls.QuestItemRenderer;
   
   public class MessageQuestInfo extends MessageInfoBase
   {
      
      public var questRender:QuestItemRenderer = new SrvQuestItemInfo();
      
      public var btnAccept:Button;
      
      public function MessageQuestInfo()
      {
         super();
         addChild(this.questRender);
         if(!this.btnAccept)
         {
            btnAccept = new SrvButtonSkinned();
            addChild(this.btnAccept);
         }
         this.btnAccept.y = 285 - this.btnAccept.height - 10;
         this.btnAccept.x = (579 - this.btnAccept.width) / 2;
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         this.btnAccept.addEventListener(ButtonEvent.CLICK,onMessageAccept,false,0,true);
         if(_data)
         {
            _data.useMultiplier = false;
            this.questRender.setData(_data);
            this.btnAccept.label = "OK";
         }
      }
   }
}

