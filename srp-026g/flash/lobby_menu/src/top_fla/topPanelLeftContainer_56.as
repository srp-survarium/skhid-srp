package top_fla
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
   import survarium.data.LocalizationData;
   
   public dynamic class topPanelLeftContainer_56 extends MovieClip
   {
      
      public var btn_add_regular_money:SrvButtonTop;
      
      public var top_rank_icon:MovieClip;
      
      public var hintButton:hitnArea;
      
      public var txLevel:MovieClip;
      
      public var progress_indicator:TopFactionProgressIndicator;
      
      public var btn_add_premium_money:SrvButtonTopPrem;
      
      public var btnChangeName:BtnChangeName;
      
      public var textFieldName:FixedTextTopNickname2;
      
      // the lobby button (StatusPanel.LOBBY_WINDOW), in the premium button's place
      public var btn_lobby:SrvButtonTop;
      
      public function topPanelLeftContainer_56()
      {
         super();
         this.__setProp_hintButton_topPanelLeftContainer_();
         this.__setProp_btn_add_regular_money_topPanelLeftContainer_();
         this.__setProp_btn_add_premium_money_topPanelLeftContainer_();
         this.__setProp_progress_indicator_topPanelLeftContainer_();
         this.placeLobbyButton();
      }
      
      // The owner (2026-10-31): "remove premium button and put lobby button there instead". The
      // premium button (the original's, `btn_premium`, which the timeline places) is taken out and
      // the lobby button takes its place, as wide: a money box of this panel (SrvButtonTop, the
      // silver's frame, hover and press), the dictionary's label centred where the amount is, no
      // coin.
      private function placeLobbyButton() : void
      {
         var slot:DisplayObject = getChildByName("btn_premium");
         var button:SrvButtonTop = new SrvButtonTop();
         button.name = "btn_lobby";
         button.x = slot.x;
         button.y = slot.y;
         button.width = slot.width;
         var content:MovieClip = button.getChildByName("content") as MovieClip;
         content.removeChild(content.getChildByName("icon"));
         var label:TextField = content.getChildByName("txMoney") as TextField;
         var format:TextFormat = label.getTextFormat();
         format.align = TextFormatAlign.CENTER;
         label.defaultTextFormat = format;
         label.x = -content.x;
         label.width = slot.width;
         label.mouseEnabled = false;
         addChildAt(button,getChildIndex(slot));
         removeChild(slot);
         this.btn_lobby = button;
         LocalizationData.addCallback(this.lobbyLabel,"st_srp_lobby_title");
      }
      
      private function lobbyLabel(text:String) : void
      {
         TextField(MovieClip(this.btn_lobby.getChildByName("content")).getChildByName("txMoney")).text = text;
      }
      
      internal function __setProp_hintButton_topPanelLeftContainer_() : *
      {
         try
         {
            this.hintButton["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         this.hintButton.enabled = true;
         this.hintButton.enableInitCallback = false;
         this.hintButton.tooltipRendererName = "SrvProfileTipRenderer";
         this.hintButton.visible = true;
         try
         {
            this.hintButton["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_btn_add_regular_money_topPanelLeftContainer_() : *
      {
         try
         {
            this.btn_add_regular_money["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         this.btn_add_regular_money.autoRepeat = false;
         this.btn_add_regular_money.autoSize = "none";
         this.btn_add_regular_money.data = "";
         this.btn_add_regular_money.enabled = true;
         this.btn_add_regular_money.enableInitCallback = false;
         this.btn_add_regular_money.focusable = true;
         this.btn_add_regular_money.label = "label";
         this.btn_add_regular_money.selected = false;
         this.btn_add_regular_money.tooltipRendererName = "";
         this.btn_add_regular_money.toggle = false;
         this.btn_add_regular_money.visible = true;
         try
         {
            this.btn_add_regular_money["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_btn_add_premium_money_topPanelLeftContainer_() : *
      {
         try
         {
            this.btn_add_premium_money["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         this.btn_add_premium_money.autoRepeat = false;
         this.btn_add_premium_money.autoSize = "none";
         this.btn_add_premium_money.data = "";
         this.btn_add_premium_money.enabled = true;
         this.btn_add_premium_money.enableInitCallback = false;
         this.btn_add_premium_money.focusable = true;
         this.btn_add_premium_money.label = "label";
         this.btn_add_premium_money.selected = false;
         this.btn_add_premium_money.tooltipRendererName = "";
         this.btn_add_premium_money.toggle = false;
         this.btn_add_premium_money.visible = true;
         try
         {
            this.btn_add_premium_money["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_progress_indicator_topPanelLeftContainer_() : *
      {
         try
         {
            this.progress_indicator["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         this.progress_indicator.enabled = true;
         this.progress_indicator.enableInitCallback = false;
         this.progress_indicator.maximum = 100;
         this.progress_indicator.minimum = 0;
         this.progress_indicator.value = 0;
         this.progress_indicator.visible = true;
         try
         {
            this.progress_indicator["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
   }
}

