package
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
   import survarium.buttons.ButtonNotificationLevelLog;
   
   public dynamic class PlayerRankNotif extends ButtonNotificationLevelLog
   {
      
      public function PlayerRankNotif()
      {
         super();
         this.__setProp_rewardBar_PlayerRankNotif_();
      }
      
      internal function __setProp_rewardBar_PlayerRankNotif_() : *
      {
         try
         {
            rewardBar["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         rewardBar.autoSize = "none";
         rewardBar.buttonWidth = 0;
         rewardBar.direction = "horizontal";
         rewardBar.enabled = true;
         rewardBar.enableInitCallback = false;
         rewardBar.focusable = true;
         rewardBar.itemRendererName = "SrvRewardButtonShared";
         rewardBar.spacing = 0;
         rewardBar.visible = true;
         try
         {
            rewardBar["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
   }
}

