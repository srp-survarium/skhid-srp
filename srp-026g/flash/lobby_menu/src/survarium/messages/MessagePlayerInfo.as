package survarium.messages
{
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.text.TextField;
   import flash.utils.getDefinitionByName;
   import scaleform.clik.controls.Button;
   import scaleform.clik.events.ButtonEvent;
   import survarium.constants.Events;
   import survarium.data.LocalizationData;
   import survarium.interfaces.IPlayerProfile;
   import survarium.player.ProfileSelector;
   
   public class MessagePlayerInfo extends MessageInfoBase
   {
      
      public var btnAccept:Button;
      
      public var playerProfile:IPlayerProfile;
      
      public var profileSelector:ProfileSelector;
      
      public var textFieldLabel:TextField;
      
      public var textFieldNickname:TextField;
      
      public function MessagePlayerInfo()
      {
         super();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         var profileSelectorRendrer:Class = getDefinitionByName("SrvProfileSelector") as Class;
         profileSelector = new profileSelectorRendrer();
         this.profileSelector.local = false;
         (this.playerProfile as MovieClip).addChild(this.profileSelector);
         this.playerProfile.registerInventory(_data.inventoryList);
         this.profileSelector.setupProfiles(_data.profiles);
         this.profileSelector.addEventListener(Events.PROFILE_SELECTED,this.onProfileSelected);
         this.profileSelector.selectProfile(0);
         this.profileSelector.x = 9;
         this.profileSelector.y = 42;
         this.btnAccept.label = LocalizationData.getName("close");
         this.textFieldLabel.text = LocalizationData.getName("player_info_label");
         this.btnAccept.addEventListener(ButtonEvent.CLICK,onMessageAccept,false,0,true);
         this.textFieldNickname.text = _data.nickname;
      }
      
      private function onProfileSelected(e:Event) : void
      {
         trace("onProfileSelected");
         this.playerProfile.fillProfileItems(this.profileSelector.getSelectedProfileData());
      }
   }
}

