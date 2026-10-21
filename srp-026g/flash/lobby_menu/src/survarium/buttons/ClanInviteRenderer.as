package survarium.buttons
{
   import flash.display.MovieClip;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import scaleform.clik.controls.ListItemRenderer;
   import scaleform.clik.events.ButtonEvent;
   import survarium.data.LocalizationData;
   
   public class ClanInviteRenderer extends ListItemRenderer
   {
      
      protected static const statusMap:Array = [{
         label:"clan_invite_accepted",
         color:26112
      },{
         label:"clan_invite_rejected",
         color:10039349
      },{
         label:"clan_invite_pending",
         color:4934475
      }];
      
      public var btnCancel:MovieClip;
      
      public var textFieldStatus:TextField;
      
      public function ClanInviteRenderer()
      {
         super();
      }
      
      override protected function updateText() : void
      {
         super.updateText();
         if(_data)
         {
            visible = true;
            mouseChildren = true;
            textField.mouseEnabled = false;
            this.textFieldStatus.mouseEnabled = false;
            this.btnCancel.visible = _data.cancelable;
            this.btnCancel.enabled = this.btnCancel.visible;
            textField.text = _data.name;
            this.textFieldStatus.text = LocalizationData.getName(statusMap[_data.status].label);
            this.textFieldStatus.textColor = statusMap[_data.status].color;
         }
         else
         {
            visible = false;
         }
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         preventAutosizing = true;
         this.btnCancel.addEventListener(ButtonEvent.CLICK,this.onCancelClicked);
      }
      
      private function onCancelClicked(param1:ButtonEvent) : void
      {
         ExternalInterface.call("cancel_player_clan_ivite",_data.id);
      }
   }
}

