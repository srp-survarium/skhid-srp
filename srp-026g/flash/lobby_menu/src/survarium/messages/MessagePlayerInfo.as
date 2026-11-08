package survarium.messages
{
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.text.TextField;
   import flash.utils.getDefinitionByName;
   import scaleform.clik.controls.Button;
   import scaleform.clik.events.ButtonEvent;
   import survarium.constants.Events;
   import survarium.controls.PlayerInfo;
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

      // srp's Matches tab: the button left of Close, and its panel over the equipment while open
      protected var _matchesButton:Button;

      protected var _matches:PlayerMatches;
      
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
         this._matchesButton = new (Object(this.btnAccept).constructor as Class)() as Button;
         this._matchesButton.name = "srpPlayerMatchesTab";
         this._matchesButton.label = LocalizationData.getName("st_srp_player_matches");
         this._matchesButton.y = this.btnAccept.y;
         this._matchesButton.width = Math.max(this.btnAccept.width,int(this._matchesButton.label.length * 8 + 30));
         this._matchesButton.x = this.btnAccept.x - 8 - this._matchesButton.width;
         this._matchesButton.addEventListener(ButtonEvent.CLICK,this.onMatchesTab,false,0,true);
         addChild(this._matchesButton);
      }

      // Matches <-> Equipment: the panel over the equipment view, asked anew each time it opens.
      protected function onMatchesTab(e:ButtonEvent) : void
      {
         var area:MovieClip = this.playerProfile as MovieClip;
         if(this._matches)
         {
            this._matches.hide();
            removeChild(this._matches);
            this._matches = null;
            this._matchesButton.label = LocalizationData.getName("st_srp_player_matches");
            return;
         }
         this._matches = new PlayerMatches(String(_data.nickname),_data.nickname == PlayerInfo.nickname,area.width,area.height,
            this.btnAccept.textField.getTextFormat().font);
         this._matches.x = area.x;
         this._matches.y = area.y;
         addChild(this._matches);
         this._matches.show();
         this._matchesButton.label = LocalizationData.getName("st_srp_player_equipment");
      }
      
      private function onProfileSelected(e:Event) : void
      {
         trace("onProfileSelected");
         this.playerProfile.fillProfileItems(this.profileSelector.getSelectedProfileData());
      }
   }
}

