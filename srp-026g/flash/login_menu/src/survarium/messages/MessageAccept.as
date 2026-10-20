package survarium.messages
{
   import flash.utils.getDefinitionByName;
   import scaleform.clik.controls.Button;
   import scaleform.clik.events.ButtonEvent;
   import survarium.data.LocalizationData;
   
   public class MessageAccept extends MessageAcceptBase
   {
      
      public var btnAccept:Button;
      
      public var btnCancel:Button;
      
      public var btnClose:Button;
      
      public function MessageAccept()
      {
         var rendererAccept:Class;
         var rendererCancel:Class;
         super();
         if(!this.btnAccept)
         {
            rendererAccept = getDefinitionByName("SrvButtonSkinned") as Class;
            btnAccept = new rendererAccept();
         }
         if(!this.btnCancel)
         {
            rendererCancel = getDefinitionByName("SrvButtonSkinned") as Class;
            btnCancel = new rendererCancel();
         }
      }
      
      override protected function configUI() : void
      {
         this.btnAccept.label = LocalizationData.getName("ok").toUpperCase();
         this.btnCancel.label = LocalizationData.getName("cancel").toUpperCase();
         super.configUI();
         this.btnAccept.addEventListener(ButtonEvent.CLICK,onMessageAccept,false,0,true);
         this.btnCancel.addEventListener(ButtonEvent.CLICK,onMessageCancel,false,0,true);
         if(this.btnClose)
         {
            this.btnClose.addEventListener(ButtonEvent.CLICK,onMessageCancel,false,0,true);
         }
         tabEnabled = true;
      }
   }
}

