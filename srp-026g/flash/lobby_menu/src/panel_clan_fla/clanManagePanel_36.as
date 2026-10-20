package panel_clan_fla
{
   import adobe.utils.*;
   import flash.accessibility.*;
   import flash.desktop.*;
   import flash.display.*;
   import flash.errors.*;
   import flash.events.*;
   import flash.external.*;
   import flash.filters.*;
   import flash.geom.*;
   import flash.globalization.*;
   import flash.media.*;
   import flash.net.*;
   import flash.net.drm.*;
   import flash.printing.*;
   import flash.profiler.*;
   import flash.sampler.*;
   import flash.sensors.*;
   import flash.system.*;
   import flash.text.*;
   import flash.text.engine.*;
   import flash.text.ime.*;
   import flash.ui.*;
   import flash.utils.*;
   import flash.xml.*;
   
   public dynamic class clanManagePanel_36 extends MovieClip
   {
      
      public var textFieldRank:TextField;
      
      public var clanPlayersList:DefaultScrollingListCl;
      
      public var btnRankUp:SrvButtonSkinnedCl;
      
      public var textFieldPlayers:TextField;
      
      public var btnInviteMember:SrvButtonSkinnedCl;
      
      public var btnKickMember:SrvButtonSkinnedCl;
      
      public var btnRankDown:SrvButtonSkinnedCl;
      
      public var searchComponent:SrvPlayerSearchComponentClan;
      
      public var textFieldName:TextField;
      
      public var textFieldStatus:TextField;
      
      public var textFieldLevel:TextField;
      
      public var btnLeaveClan:SrvButtonSkinnedCl;
      
      public function clanManagePanel_36()
      {
         super();
         this.__setProp_clanPlayersList_clanManagePanel_();
      }
      
      internal function __setProp_clanPlayersList_clanManagePanel_() : *
      {
         try
         {
            this.clanPlayersList["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         this.clanPlayersList.enabled = true;
         this.clanPlayersList.enableInitCallback = false;
         this.clanPlayersList.focusable = true;
         this.clanPlayersList.itemRendererName = "SrvClanListRenderer";
         this.clanPlayersList.itemRendererInstanceName = "";
         this.clanPlayersList.margin = 0;
         this.clanPlayersList.inspectablePadding = {
            "top":0,
            "right":0,
            "bottom":0,
            "left":0
         };
         this.clanPlayersList.rowHeight = 32;
         this.clanPlayersList.scrollBar = "SrvScrollBarSkinned";
         this.clanPlayersList.visible = true;
         this.clanPlayersList.wrapping = "normal";
         try
         {
            this.clanPlayersList["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
   }
}

